//! v0.4.x flat-file migration logic.

use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value as JsonValue;

use crate::{
  AppContext,
  cli::Error,
  store::{
    model::primitives::{AuthorType, EntityType, Id, RelationshipType, TaskStatus},
    repo,
  },
};

/// Counters for reporting migration results.
#[derive(Default)]
struct MigrationCounts {
  /// Tasks imported.
  tasks: usize,
  /// Artifacts imported.
  artifacts: usize,
  /// Iterations imported.
  iterations: usize,
  /// Notes imported from tasks and iterations.
  notes: usize,
  /// Links imported; a link that could not be imported is not counted.
  relationships: usize,
  /// Files that could not be parsed and were left out.
  skipped: usize,
}

/// Run the v0.4 migration from flat-file `.gest/` directory into SQLite.
///
/// Creates a project for the current directory when there is none, imports tasks, then
/// artifacts, then iterations, and prints the counts.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `source` - the legacy `.gest/` directory to read
///
/// # Returns
///
/// `Ok(())` once the counts are printed, [`Error::NotFound`] when `source` is not a directory,
/// or the first I/O or store error.
pub async fn run(context: &AppContext, source: &Path) -> Result<(), Error> {
  if !source.is_dir() {
    return Err(Error::NotFound(format!(
      "source directory does not exist: {}",
      source.display()
    )));
  }

  let cwd = std::env::current_dir()?;
  let conn = context.store().connect().await?;

  let project_id = match context.project_id() {
    Some(id) => id.clone(),
    None => {
      let project = repo::project::create(&conn, &cwd).await?;
      project.id().clone()
    }
  };

  let mut counts = MigrationCounts::default();

  migrate_all(&conn, &project_id, source, &mut counts).await?;

  log::info!(
    "migrated {} tasks, {} artifacts, {} iterations, {} notes, {} relationships",
    counts.tasks,
    counts.artifacts,
    counts.iterations,
    counts.notes,
    counts.relationships,
  );
  println!(
    "migrated {} tasks, {} artifacts, {} iterations, {} notes, {} relationships",
    counts.tasks, counts.artifacts, counts.iterations, counts.notes, counts.relationships,
  );
  if counts.skipped > 0 {
    println!("skipped {} files (the warnings above say why)", counts.skipped);
  }
  Ok(())
}

/// A legacy link waiting for every entity to exist, so its target can be looked up by its legacy ID.
struct PendingLink {
  /// The kind of the entity that holds the link.
  source_type: EntityType,
  /// The new ID of the entity that holds the link.
  source_id: Id,
  /// The link as the legacy file wrote it.
  link: LegacyLink,
}

/// Legacy IDs mapped to new IDs, one map for each kind of entity.
struct IdMaps {
  /// Legacy task IDs.
  tasks: HashMap<String, Id>,
  /// Legacy artifact IDs.
  artifacts: HashMap<String, Id>,
  /// Legacy iteration IDs.
  iterations: HashMap<String, Id>,
}

/// Migrate tasks, artifacts and iterations, then their links.
///
/// Links are imported last, so a link can name any entity, including one read after the entity that holds the link.
/// v0.4 wrote a link on both entities, the second as the inverse; [`import_link`] keeps one row for each pair, as
/// `gest task link` makes, except between two iterations, where `gest iteration link` also makes both rows.
///
/// # Arguments
///
/// * `conn` - the connection to write to
/// * `project_id` - the project that owns the imported entities
/// * `source` - the legacy `.gest` directory
/// * `counts` - the running totals, updated in place
///
/// # Returns
///
/// `Ok(())` once everything is imported, or the first database or I/O error.
async fn migrate_all(
  conn: &libsql::Connection,
  project_id: &Id,
  source: &Path,
  counts: &mut MigrationCounts,
) -> Result<(), Error> {
  let mut links = Vec::new();
  let tasks = migrate_tasks(conn, project_id, source, counts, &mut links).await?;
  let artifacts = migrate_artifacts(conn, project_id, source, counts).await?;
  let iterations = migrate_iterations(conn, project_id, source, counts, &tasks, &mut links).await?;
  let maps = IdMaps {
    tasks,
    artifacts,
    iterations,
  };

  let mut imported = HashSet::new();
  for pending in &links {
    if import_link(conn, pending, &maps, &mut imported).await? {
      counts.relationships += 1;
    }
  }
  Ok(())
}

/// Migrate tasks and return a mapping of old ID string → new Id.
///
/// Reads the `.toml` files in `tasks/` and `tasks/resolved/`, with their tags, notes and links.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project that receives the imported entities
/// * `source` - the legacy `.gest/` directory
/// * `counts` - the running totals, updated for each entity, note and skipped file
/// * `links` - the links waiting for every entity to exist; this function adds to it
///
/// # Returns
///
/// Each legacy task ID mapped to its new ID, or the first I/O or store error. A file that does
/// not parse is skipped and counted.
async fn migrate_tasks(
  conn: &libsql::Connection,
  project_id: &Id,
  source: &Path,
  counts: &mut MigrationCounts,
  links: &mut Vec<PendingLink>,
) -> Result<HashMap<String, Id>, Error> {
  let mut id_map = HashMap::new();

  for dir in [source.join("tasks"), source.join("tasks/resolved")] {
    if !dir.is_dir() {
      continue;
    }
    for path in read_files_with_ext(&dir, "toml")? {
      let content = std::fs::read_to_string(&path)?;
      match toml::from_str::<LegacyTask>(&content) {
        Ok(task) => {
          let old_id = task.id.clone();
          let assigned_to = match &task.assigned_to {
            Some(name) => {
              let author = repo::author::find_or_create(conn, name, None, AuthorType::Human).await?;
              Some(author.id().clone())
            }
            None => None,
          };

          let status: Option<TaskStatus> = task.status.parse().ok();
          let metadata = toml_table_to_json(&task.metadata);

          let created = repo::task::create(
            conn,
            project_id,
            &crate::store::model::task::New {
              assigned_to,
              description: task.description.unwrap_or_default(),
              metadata,
              priority: task.priority,
              status,
              title: task.title,
            },
          )
          .await?;

          let new_id = created.id().clone();

          // Tags
          for tag in task.tags.unwrap_or_default() {
            repo::tag::attach(conn, EntityType::Task, &new_id, &tag).await?;
          }

          // Notes
          for note in task.notes.unwrap_or_default() {
            let author_id = match &note.author {
              Some(name) => {
                let author_type = match note.author_type.as_deref() {
                  Some("agent") => AuthorType::Agent,
                  _ => AuthorType::Human,
                };
                let author =
                  repo::author::find_or_create(conn, name, note.author_email.as_deref(), author_type).await?;
                Some(author.id().clone())
              }
              None => None,
            };
            repo::note::create(
              conn,
              EntityType::Task,
              &new_id,
              &crate::store::model::note::New {
                author_id,
                body: note.body,
              },
            )
            .await?;
            counts.notes += 1;
          }

          // Links wait until every entity exists.
          for link in task.links.unwrap_or_default() {
            links.push(PendingLink {
              source_type: EntityType::Task,
              source_id: new_id.clone(),
              link,
            });
          }

          id_map.insert(old_id, new_id);
          counts.tasks += 1;
        }
        Err(e) => {
          log::warn!("skipping {}: {e}", path.display());
          counts.skipped += 1;
        }
      }
    }
  }

  Ok(id_map)
}

/// Migrate the markdown artifacts in `artifacts/` and `artifacts/archive/`.
///
/// Archived artifacts are archived again, and an artifact's legacy `type` becomes one more tag.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project that receives the imported entities
/// * `source` - the legacy `.gest/` directory
/// * `counts` - the running totals, updated for each artifact and skipped file
///
/// # Returns
///
/// Legacy artifact IDs mapped to their new IDs, or the first I/O or store error. A file that does
/// not parse is skipped and counted.
async fn migrate_artifacts(
  conn: &libsql::Connection,
  project_id: &Id,
  source: &Path,
  counts: &mut MigrationCounts,
) -> Result<HashMap<String, Id>, Error> {
  let mut id_map = HashMap::new();
  for (dir, archived) in [
    (source.join("artifacts"), false),
    (source.join("artifacts/archive"), true),
  ] {
    if !dir.is_dir() {
      continue;
    }
    for path in read_files_with_ext(&dir, "md")? {
      let content = std::fs::read_to_string(&path)?;
      match parse_legacy_artifact(&content, archived) {
        Ok(artifact) => {
          let created = repo::artifact::create(
            conn,
            project_id,
            &crate::store::model::artifact::New {
              body: artifact.body,
              metadata: None,
              title: artifact.title,
            },
          )
          .await?;

          let new_id = created.id().clone();
          if let Some(old_id) = artifact.id.clone().or_else(|| file_stem(&path)) {
            id_map.insert(old_id, new_id.clone());
          }

          // Archive if needed
          if artifact.archived_at.is_some() {
            repo::artifact::archive(conn, &new_id).await?;
          }

          // Tags (including kind as a tag)
          let mut tags = artifact.tags;
          if let Some(kind) = artifact.kind
            && !tags.contains(&kind)
          {
            tags.push(kind);
          }
          for tag in &tags {
            repo::tag::attach(conn, EntityType::Artifact, &new_id, tag).await?;
          }

          counts.artifacts += 1;
        }
        Err(e) => {
          log::warn!("skipping {}: {e}", path.display());
          counts.skipped += 1;
        }
      }
    }
  }

  Ok(id_map)
}

/// Migrate the iterations in `iterations/` and `iterations/resolved/`, with their tags, tasks,
/// notes and links.
///
/// Every task joins phase 1; a task ref that was not migrated is logged and left out. An artifact
/// ref in the task list becomes a `relates-to` link. Links are collected into `links`.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project that receives the imported entities
/// * `source` - the legacy `.gest/` directory
/// * `counts` - the running totals, updated for each entity, note, link and skipped file
/// * `task_id_map` - legacy task IDs mapped to their new IDs, from [`migrate_tasks`]
/// * `links` - the links waiting for every entity to exist; this function adds to it
///
/// # Returns
///
/// Legacy iteration IDs mapped to their new IDs, or the first I/O or store error. A file that
/// does not parse is skipped and counted.
async fn migrate_iterations(
  conn: &libsql::Connection,
  project_id: &Id,
  source: &Path,
  counts: &mut MigrationCounts,
  task_id_map: &HashMap<String, Id>,
  links: &mut Vec<PendingLink>,
) -> Result<HashMap<String, Id>, Error> {
  let mut id_map = HashMap::new();
  for dir in [source.join("iterations"), source.join("iterations/resolved")] {
    if !dir.is_dir() {
      continue;
    }
    for path in read_files_with_ext(&dir, "toml")? {
      let content = std::fs::read_to_string(&path)?;
      match toml::from_str::<LegacyIteration>(&content) {
        Ok(iteration) => {
          let metadata = toml_table_to_json(&iteration.metadata);

          let created = repo::iteration::create(
            conn,
            project_id,
            &crate::store::model::iteration::New {
              description: iteration.description.unwrap_or_default(),
              metadata,
              title: iteration.title,
            },
          )
          .await?;

          let new_id = created.id().clone();
          if let Some(old_id) = iteration.id.clone().or_else(|| file_stem(&path)) {
            id_map.insert(old_id, new_id.clone());
          }

          // Tags
          for tag in iteration.tags.unwrap_or_default() {
            repo::tag::attach(conn, EntityType::Iteration, &new_id, &tag).await?;
          }

          // Task associations
          for task_ref in iteration.tasks.unwrap_or_default() {
            // v0.4 `artifact create --iteration` listed the artifact here; it becomes a link.
            if task_ref.starts_with("artifacts/") {
              links.push(PendingLink {
                source_type: EntityType::Iteration,
                source_id: new_id.clone(),
                link: LegacyLink {
                  ref_: task_ref,
                  rel: "relates-to".to_string(),
                },
              });
              continue;
            }
            let task_id_str = task_ref.strip_prefix("tasks/").unwrap_or(&task_ref);
            if let Some(new_task_id) = task_id_map.get(task_id_str) {
              repo::iteration::add_task(conn, &new_id, new_task_id, 1).await?;
            } else {
              log::warn!("iteration task ref {task_ref} not found in migrated tasks");
            }
          }

          // Notes
          for note in iteration.notes.unwrap_or_default() {
            let author_id = match &note.author {
              Some(name) => {
                let author_type = match note.author_type.as_deref() {
                  Some("agent") => AuthorType::Agent,
                  _ => AuthorType::Human,
                };
                let author =
                  repo::author::find_or_create(conn, name, note.author_email.as_deref(), author_type).await?;
                Some(author.id().clone())
              }
              None => None,
            };
            repo::note::create(
              conn,
              EntityType::Iteration,
              &new_id,
              &crate::store::model::note::New {
                author_id,
                body: note.body,
              },
            )
            .await?;
            counts.notes += 1;
          }

          // Links wait until every entity exists.
          for link in iteration.links.unwrap_or_default() {
            links.push(PendingLink {
              source_type: EntityType::Iteration,
              source_id: new_id.clone(),
              link,
            });
          }

          counts.iterations += 1;
        }
        Err(e) => {
          log::warn!("skipping {}: {e}", path.display());
          counts.skipped += 1;
        }
      }
    }
  }

  Ok(id_map)
}

/// The file name of `path` without its extension, which is the legacy ID of the entity in it.
fn file_stem(path: &Path) -> Option<String> {
  path.file_stem().and_then(|stem| stem.to_str()).map(str::to_string)
}

/// Import a v0.4 link as a relationship. Links reference entities by path like
/// "tasks/<id>" or "artifacts/<id>".
///
/// An unknown rel, a ref that does not resolve, and a failed insert are each logged and skipped,
/// so one bad link does not stop the migration. A link whose inverse row this import already made
/// is skipped quietly, unless both ends are iterations.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `pending` - the link and the entity that holds it
/// * `maps` - legacy IDs mapped to new IDs
/// * `imported` - the rows made so far, as [`row_key`] writes them; updated in place
///
/// # Returns
///
/// `Ok(true)` when the link became a row, `Ok(false)` when it was skipped.
async fn import_link(
  conn: &libsql::Connection,
  pending: &PendingLink,
  maps: &IdMaps,
  imported: &mut HashSet<String>,
) -> Result<bool, Error> {
  let (source_type, source_id, link) = (pending.source_type, &pending.source_id, &pending.link);
  let rel_type: RelationshipType = match link.rel.parse() {
    Ok(r) => r,
    Err(_) => {
      log::warn!("unknown relationship type: {}", link.rel);
      return Ok(false);
    }
  };

  let (target_type, target_id) = match resolve_link_ref(&link.ref_, maps) {
    Some(pair) => pair,
    None => {
      log::warn!("could not resolve link ref: {}", link.ref_);
      return Ok(false);
    }
  };

  let both_iterations = source_type == EntityType::Iteration && target_type == EntityType::Iteration;
  if !both_iterations
    && imported.contains(&row_key(
      target_type,
      &target_id,
      rel_type.inverse(),
      source_type,
      source_id,
    ))
  {
    log::debug!(
      "link {} {} is the inverse of a link already imported",
      link.rel,
      link.ref_
    );
    return Ok(false);
  }

  match repo::relationship::create(conn, rel_type, source_type, source_id, target_type, &target_id).await {
    Ok(_) => {
      imported.insert(row_key(source_type, source_id, rel_type, target_type, &target_id));
      Ok(true)
    }
    Err(e) => {
      log::warn!("could not import link {} {}: {e}", link.rel, link.ref_);
      Ok(false)
    }
  }
}

/// A relationship row as one string, so the rows an import made can be looked up.
fn row_key(
  source_type: EntityType,
  source_id: &Id,
  rel: RelationshipType,
  target_type: EntityType,
  target_id: &Id,
) -> String {
  format!("{source_type}/{source_id} {rel} {target_type}/{target_id}")
}

/// Resolve a v0.4 link ref (e.g. "tasks/<id>", "artifacts/<id>") to an entity type and new ID.
///
/// # Arguments
///
/// * `ref_` - the legacy ref, a kind folder and an ID
/// * `maps` - legacy IDs mapped to new IDs
///
/// # Returns
///
/// The entity type and new ID, or `None` for an unknown kind or an entity that was not migrated.
fn resolve_link_ref(ref_: &str, maps: &IdMaps) -> Option<(EntityType, Id)> {
  let (entity_type, map, legacy_id) = if let Some(id) = ref_.strip_prefix("tasks/") {
    (EntityType::Task, &maps.tasks, id)
  } else if let Some(id) = ref_.strip_prefix("artifacts/") {
    (EntityType::Artifact, &maps.artifacts, id)
  } else {
    (
      EntityType::Iteration,
      &maps.iterations,
      ref_.strip_prefix("iterations/")?,
    )
  };
  map.get(legacy_id).map(|new_id| (entity_type, new_id.clone()))
}

/// Convert a TOML table to a JSON value for metadata storage.
///
/// # Arguments
///
/// * `table` - the legacy `metadata` table
///
/// # Returns
///
/// The JSON object, or `None` when the table is empty or does not convert.
fn toml_table_to_json(table: &toml::Table) -> Option<JsonValue> {
  if table.is_empty() {
    return None;
  }
  // TOML Value -> JSON Value via serde
  let toml_value = toml::Value::Table(table.clone());
  serde_json::to_value(toml_value).ok()
}

/// List the files in `dir` with the extension `ext`, not looking into subdirectories.
///
/// # Arguments
///
/// * `dir` - the directory to list
/// * `ext` - the extension without its dot, such as `toml`
///
/// # Returns
///
/// The paths, sorted; empty when `dir` does not exist.
fn read_files_with_ext(dir: &Path, ext: &str) -> std::io::Result<Vec<PathBuf>> {
  let mut paths = Vec::new();
  if !dir.exists() {
    return Ok(paths);
  }
  for entry in std::fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some(ext) {
      paths.push(path);
    }
  }
  paths.sort();
  Ok(paths)
}

/// Parse a legacy artifact file (YAML frontmatter + markdown body).
///
/// # Arguments
///
/// * `content` - the whole file
/// * `archived` - whether the file came from `artifacts/archive/`
///
/// # Returns
///
/// The artifact, or a message naming the missing delimiter or field (`title`, `created_at` or
/// `updated_at`). An archived artifact with no `archived_at` takes its `updated_at`.
fn parse_legacy_artifact(content: &str, archived: bool) -> Result<LegacyArtifact, String> {
  let stripped = content
    .strip_prefix("---\n")
    .ok_or("missing opening frontmatter delimiter")?;
  let end = stripped
    .find("\n---\n")
    .ok_or("missing closing frontmatter delimiter")?;
  let yaml = &stripped[..end];
  let rest = &stripped[end + 5..];
  let body = rest.strip_prefix('\n').unwrap_or(rest);

  let fields = parse_yaml_frontmatter(yaml);

  let title = fields.get("title").ok_or("missing title")?.clone();
  let created_at = fields.get("created_at").ok_or("missing created_at")?.clone();
  let updated_at = fields.get("updated_at").ok_or("missing updated_at")?.clone();
  let kind = fields.get("type").cloned();
  let tags = parse_yaml_list_field(&fields, "tags");

  let archived_at = if archived {
    fields
      .get("archived_at")
      .filter(|s| !s.is_empty())
      .cloned()
      .or_else(|| Some(updated_at.clone()))
  } else {
    None
  };

  Ok(LegacyArtifact {
    archived_at,
    id: fields.get("id").filter(|id| !id.is_empty()).cloned(),
    body: body.to_string(),
    _created_at: created_at,
    kind,
    tags,
    title,
    _updated_at: updated_at,
  })
}

/// Read the flat `key: value` pairs of a frontmatter block, a small subset of YAML.
///
/// Quotes around a value are removed. A key with an empty value followed by `- item` lines
/// holds the items joined with commas.
///
/// # Arguments
///
/// * `yaml` - the text between the `---` delimiters
///
/// # Returns
///
/// Each key mapped to its value as a string. A key with an empty value and no `- item`
/// lines is left out.
fn parse_yaml_frontmatter(yaml: &str) -> HashMap<String, String> {
  let mut fields = HashMap::new();
  let mut current_key: Option<String> = None;
  let mut list_values: Vec<String> = Vec::new();

  for line in yaml.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }

    if let Some(item) = trimmed.strip_prefix("- ") {
      if let Some(ref key) = current_key {
        list_values.push(item.trim().to_string());
        fields.insert(key.clone(), list_values.join(","));
      }
      continue;
    }

    current_key = None;
    list_values.clear();

    if let Some((key, value)) = trimmed.split_once(':') {
      let key = key.trim().to_string();
      let value = value.trim();

      if value.is_empty() {
        current_key = Some(key);
        list_values.clear();
        continue;
      }

      let value = value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(value);

      fields.insert(key, value.to_string());
    }
  }

  fields
}

/// Split a frontmatter value on commas into a list.
///
/// # Arguments
///
/// * `fields` - the pairs from [`parse_yaml_frontmatter`]
/// * `key` - the key to read, such as `tags`
///
/// # Returns
///
/// The trimmed, non-empty items; empty when `key` is missing.
fn parse_yaml_list_field(fields: &HashMap<String, String>, key: &str) -> Vec<String> {
  fields
    .get(key)
    .map(|s| {
      s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
    })
    .unwrap_or_default()
}

/// Parsed legacy artifact data.
struct LegacyArtifact {
  /// When it was archived; set only for files in `artifacts/archive/`.
  archived_at: Option<String>,
  /// The markdown after the frontmatter.
  body: String,
  /// The `created_at` frontmatter value; required but not imported.
  _created_at: String,
  /// The legacy ID from the frontmatter, which links refer to; the file name when absent.
  id: Option<String>,
  /// The legacy `type` field, imported as a tag.
  kind: Option<String>,
  /// Tag labels from the `tags` list.
  tags: Vec<String>,
  /// The `title` frontmatter value.
  title: String,
  /// The `updated_at` frontmatter value; required, and used as `archived_at` when that is missing.
  _updated_at: String,
}

/// A note embedded in a v0.4 task or iteration TOML file.
#[derive(Debug, Deserialize)]
struct LegacyNote {
  /// The author's name; with none, the note has no author.
  #[serde(default)]
  author: Option<String>,
  /// The author's email, if recorded.
  #[serde(default)]
  author_email: Option<String>,
  /// `agent` for an agent author; anything else means a human.
  #[serde(default)]
  author_type: Option<String>,
  /// The note text.
  body: String,
}

/// A link/relationship embedded in a v0.4 task or iteration TOML file.
#[derive(Debug, Deserialize)]
struct LegacyLink {
  /// The target as `<kind>/<id>`, such as `tasks/abc`; the TOML key is `ref`.
  #[serde(rename = "ref")]
  ref_: String,
  /// The relationship type, such as `blocks`; an unknown one skips the link.
  rel: String,
}

/// Deserialization target for legacy task TOML files.
#[derive(Debug, Deserialize)]
struct LegacyTask {
  /// The assignee's name, imported as a human author.
  #[serde(default)]
  assigned_to: Option<String>,
  /// The task description; empty when absent.
  description: Option<String>,
  /// The legacy ID, which links and iterations refer to.
  id: String,
  /// Links to other entities.
  #[serde(default)]
  links: Option<Vec<LegacyLink>>,
  /// Free-form metadata, imported as JSON.
  #[serde(default)]
  metadata: toml::Table,
  /// Notes on the task.
  #[serde(default)]
  notes: Option<Vec<LegacyNote>>,
  /// The priority; lower numbers rank first.
  #[serde(default)]
  priority: Option<u8>,
  /// When the task was resolved; read but not imported.
  #[cfg_attr(not(test), allow(dead_code))]
  #[serde(default)]
  resolved_at: Option<String>,
  /// The status name; one that does not parse leaves the new task with the default status.
  status: String,
  /// Tag labels.
  #[serde(default)]
  tags: Option<Vec<String>>,
  /// The task title.
  title: String,
}

/// Deserialization target for legacy iteration TOML files.
#[derive(Debug, Deserialize)]
struct LegacyIteration {
  /// The iteration description; empty when absent.
  description: Option<String>,
  /// The legacy ID, which links refer to; the file name when absent.
  #[serde(default)]
  id: Option<String>,
  /// Links to other entities.
  #[serde(default)]
  links: Option<Vec<LegacyLink>>,
  /// Free-form metadata, imported as JSON.
  #[serde(default)]
  metadata: toml::Table,
  /// Notes on the iteration.
  #[serde(default)]
  notes: Option<Vec<LegacyNote>>,
  /// Tag labels.
  #[serde(default)]
  tags: Option<Vec<String>>,
  /// Task refs, as `tasks/<id>` or a bare ID.
  #[serde(default)]
  tasks: Option<Vec<String>>,
  /// The iteration title.
  title: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod migrate_integration {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;

    use super::*;

    fn setup_legacy_dir(dir: &Path) {
      for sub in [
        "tasks",
        "tasks/resolved",
        "artifacts",
        "artifacts/archive",
        "iterations",
        "iterations/resolved",
      ] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
      }
    }

    async fn setup_db() -> (Arc<crate::store::Db>, Id, tempfile::TempDir) {
      let (store, tmp) = crate::store::open_temp().await.unwrap();
      let conn = store.connect().await.unwrap();
      let cwd = std::env::current_dir().unwrap();
      let project = repo::project::create(&conn, &cwd).await.unwrap();
      (store, project.id().clone(), tmp)
    }

    #[tokio::test]
    async fn it_imports_artifacts() {
      let tmp = tempfile::tempdir().unwrap();
      let source = tmp.path().join("legacy");
      setup_legacy_dir(&source);

      let artifact_md = "---\n\
        id: zyxwvutsrqponmlkzyxwvutsrqponmlk\n\
        title: My Artifact\n\
        type: adr\n\
        tags:\n  - design\n\
        created_at: \"2026-01-01T00:00:00Z\"\n\
        updated_at: \"2026-01-02T00:00:00Z\"\n\
        ---\n\n\
        # Hello\n\nThis is the body.\n";
      std::fs::write(
        source.join("artifacts/zyxwvutsrqponmlkzyxwvutsrqponmlk.md"),
        artifact_md,
      )
      .unwrap();

      let (store, project_id, _db_tmp) = setup_db().await;
      let conn = store.connect().await.unwrap();
      let mut counts = MigrationCounts::default();
      migrate_artifacts(&conn, &project_id, &source, &mut counts)
        .await
        .unwrap();

      assert_eq!(counts.artifacts, 1);

      // Verify via repo layer
      let artifacts = repo::artifact::all(
        &conn,
        &project_id,
        &crate::store::model::artifact::Filter {
          all: true,
          ..Default::default()
        },
      )
      .await
      .unwrap();
      assert_eq!(artifacts.len(), 1);
      assert_eq!(artifacts[0].title(), "My Artifact");

      // Tags
      let tags = repo::tag::for_entity(&conn, EntityType::Artifact, artifacts[0].id())
        .await
        .unwrap();
      assert!(tags.contains(&"adr".to_string()));
      assert!(tags.contains(&"design".to_string()));
    }

    #[tokio::test]
    async fn it_imports_iterations_with_task_associations() {
      let tmp = tempfile::tempdir().unwrap();
      let source = tmp.path().join("legacy");
      setup_legacy_dir(&source);

      // Create a task first
      let task_toml = r#"
        id = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk"
        title = "Linked task"
        status = "open"
        created_at = "2026-01-01T00:00:00Z"
        updated_at = "2026-01-01T00:00:00Z"
      "#;
      std::fs::write(source.join("tasks/kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk.toml"), task_toml).unwrap();

      let iter_toml = r#"
        id = "zyxwvutsrqponmlkzyxwvutsrqponmlk"
        title = "Sprint 1"
        description = "First sprint"
        status = "active"
        tags = ["sprint"]
        tasks = ["tasks/kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk"]
        created_at = "2026-01-01T00:00:00Z"
        updated_at = "2026-01-02T00:00:00Z"
      "#;
      std::fs::write(
        source.join("iterations/zyxwvutsrqponmlkzyxwvutsrqponmlk.toml"),
        iter_toml,
      )
      .unwrap();

      let (store, project_id, _db_tmp) = setup_db().await;
      let conn = store.connect().await.unwrap();
      let mut counts = MigrationCounts::default();

      let mut links = Vec::new();
      let task_id_map = migrate_tasks(&conn, &project_id, &source, &mut counts, &mut links)
        .await
        .unwrap();
      migrate_iterations(&conn, &project_id, &source, &mut counts, &task_id_map, &mut links)
        .await
        .unwrap();

      assert_eq!(counts.tasks, 1);
      assert_eq!(counts.iterations, 1);

      // Verify iteration was created
      let iterations = repo::iteration::all(
        &conn,
        &project_id,
        &crate::store::model::iteration::Filter {
          all: true,
          ..Default::default()
        },
      )
      .await
      .unwrap();
      assert_eq!(iterations.len(), 1);
      assert_eq!(iterations[0].title(), "Sprint 1");
      assert_eq!(iterations[0].description(), "First sprint");

      // Verify task association
      let tasks = repo::iteration::tasks_with_phase(&conn, iterations[0].id())
        .await
        .unwrap();
      assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn it_points_every_link_at_the_new_ids() {
      let tmp = tempfile::tempdir().unwrap();
      let source = tmp.path().join("legacy");
      setup_legacy_dir(&source);

      let (first, later, artifact, iteration) = (
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        "mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm",
        "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn",
      );
      // The first task links forward to a task read after it, and to an artifact.
      let first_toml = format!(
        "id = \"{first}\"\ntitle = \"First\"\nstatus = \"open\"\n\
          [[links]]\nref = \"tasks/{later}\"\nrel = \"blocks\"\n\
          [[links]]\nref = \"artifacts/{artifact}\"\nrel = \"relates-to\"\n"
      );
      std::fs::write(source.join(format!("tasks/{first}.toml")), first_toml).unwrap();
      let later_toml = format!("id = \"{later}\"\ntitle = \"Later\"\nstatus = \"open\"\n");
      std::fs::write(source.join(format!("tasks/{later}.toml")), later_toml).unwrap();
      let artifact_md = format!(
        "---\nid: {artifact}\ntitle: Spec\ncreated_at: \"2026-01-01T00:00:00Z\"\n\
          updated_at: \"2026-01-01T00:00:00Z\"\n---\n\nBody\n"
      );
      std::fs::write(source.join(format!("artifacts/{artifact}.md")), artifact_md).unwrap();
      // The iteration links to the artifact; a task links back to the iteration.
      let iteration_toml = format!(
        "id = \"{iteration}\"\ntitle = \"Sprint\"\n\
          [[links]]\nref = \"artifacts/{artifact}\"\nrel = \"relates-to\"\n"
      );
      std::fs::write(source.join(format!("iterations/{iteration}.toml")), iteration_toml).unwrap();
      let back_toml = format!(
        "id = \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"\ntitle = \"Back\"\nstatus = \"open\"\n\
          [[links]]\nref = \"iterations/{iteration}\"\nrel = \"relates-to\"\n"
      );
      std::fs::write(source.join("tasks/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.toml"), back_toml).unwrap();

      let (store, project_id, _db_tmp) = setup_db().await;
      let conn = store.connect().await.unwrap();
      let mut counts = MigrationCounts::default();
      migrate_all(&conn, &project_id, &source, &mut counts).await.unwrap();

      let mut rows = conn
        .query(
          "SELECT count(*), count(CASE WHEN target_id IN \
              (SELECT id FROM tasks UNION SELECT id FROM artifacts UNION SELECT id FROM iterations) THEN 1 END) \
            FROM relationships",
          (),
        )
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      let (total, resolved): (i64, i64) = (row.get(0).unwrap(), row.get(1).unwrap());

      assert_eq!(
        (total, resolved),
        (4, 4),
        "every link should be imported and point at a new id"
      );
      assert_eq!(counts.relationships, 4);
    }

    /// The relationship rows, as `source_type rel target_type` lines, sorted.
    async fn relationship_rows(conn: &libsql::Connection) -> Vec<String> {
      let mut rows = conn
        .query("SELECT source_type, rel_type, target_type FROM relationships", ())
        .await
        .unwrap();
      let mut out = Vec::new();
      while let Some(row) = rows.next().await.unwrap() {
        let (source, rel, target): (String, String, String) =
          (row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap());
        out.push(format!("{source} {rel} {target}"));
      }
      out.sort();
      out
    }

    #[tokio::test]
    async fn it_imports_one_row_for_a_task_link_that_v0_4_wrote_on_both_tasks() {
      let tmp = tempfile::tempdir().unwrap();
      let source = tmp.path().join("legacy");
      setup_legacy_dir(&source);
      let (a, b) = ("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
      for (id, other, rel) in [(a, b, "blocks"), (b, a, "blocked-by")] {
        let toml = format!(
          "id = \"{id}\"\ntitle = \"T\"\nstatus = \"open\"\n[[links]]\nref = \"tasks/{other}\"\nrel = \"{rel}\"\n"
        );
        std::fs::write(source.join(format!("tasks/{id}.toml")), toml).unwrap();
      }

      let (store, project_id, _db_tmp) = setup_db().await;
      let conn = store.connect().await.unwrap();
      let mut counts = MigrationCounts::default();
      migrate_all(&conn, &project_id, &source, &mut counts).await.unwrap();

      assert_eq!(relationship_rows(&conn).await, vec!["task blocks task".to_string()]);
      assert_eq!(counts.relationships, 1);
    }

    #[tokio::test]
    async fn it_keeps_both_rows_of_an_iteration_pair_as_iteration_link_does() {
      let tmp = tempfile::tempdir().unwrap();
      let source = tmp.path().join("legacy");
      setup_legacy_dir(&source);
      let (a, b) = ("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
      for (id, other, rel) in [(a, b, "blocks"), (b, a, "blocked-by")] {
        let toml = format!("id = \"{id}\"\ntitle = \"I\"\n[[links]]\nref = \"iterations/{other}\"\nrel = \"{rel}\"\n");
        std::fs::write(source.join(format!("iterations/{id}.toml")), toml).unwrap();
      }

      let (store, project_id, _db_tmp) = setup_db().await;
      let conn = store.connect().await.unwrap();
      let mut counts = MigrationCounts::default();
      migrate_all(&conn, &project_id, &source, &mut counts).await.unwrap();

      assert_eq!(
        relationship_rows(&conn).await,
        vec![
          "iteration blocked-by iteration".to_string(),
          "iteration blocks iteration".to_string()
        ]
      );
    }

    #[tokio::test]
    async fn it_links_an_artifact_listed_among_an_iterations_tasks() {
      let tmp = tempfile::tempdir().unwrap();
      let source = tmp.path().join("legacy");
      setup_legacy_dir(&source);
      let artifact = "mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm";
      let artifact_md = format!(
        "---\nid: {artifact}\ntitle: Spec\ncreated_at: \"2026-01-01T00:00:00Z\"\nupdated_at: \"2026-01-01T00:00:00Z\"\n---\n\nBody\n"
      );
      std::fs::write(source.join(format!("artifacts/{artifact}.md")), artifact_md).unwrap();
      let iteration =
        format!("id = \"nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn\"\ntitle = \"Sprint\"\ntasks = [\"artifacts/{artifact}\"]\n");
      std::fs::write(
        source.join("iterations/nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn.toml"),
        iteration,
      )
      .unwrap();

      let (store, project_id, _db_tmp) = setup_db().await;
      let conn = store.connect().await.unwrap();
      let mut counts = MigrationCounts::default();
      migrate_all(&conn, &project_id, &source, &mut counts).await.unwrap();

      assert_eq!(
        relationship_rows(&conn).await,
        vec!["iteration relates-to artifact".to_string()]
      );
    }

    #[tokio::test]
    async fn it_counts_only_the_links_it_imported() {
      let tmp = tempfile::tempdir().unwrap();
      let source = tmp.path().join("legacy");
      setup_legacy_dir(&source);
      let toml = "id = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\ntitle = \"Lonely\"\nstatus = \"open\"\n\
          [[links]]\nref = \"tasks/kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\"\nrel = \"blocks\"\n";
      std::fs::write(source.join("tasks/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.toml"), toml).unwrap();

      let (store, project_id, _db_tmp) = setup_db().await;
      let conn = store.connect().await.unwrap();
      let mut counts = MigrationCounts::default();
      migrate_all(&conn, &project_id, &source, &mut counts).await.unwrap();

      assert_eq!(counts.relationships, 0);
    }

    #[tokio::test]
    async fn it_imports_tasks_with_notes_and_metadata() {
      let tmp = tempfile::tempdir().unwrap();
      let source = tmp.path().join("legacy");
      setup_legacy_dir(&source);

      let task_toml = r#"
        id = "zyxwvutsrqponmlkzyxwvutsrqponmlk"
        title = "Test task"
        description = "A description"
        status = "open"
        priority = 1
        assigned_to = "agent-1"
        tags = ["bug", "urgent"]
        created_at = "2026-01-01T00:00:00Z"
        updated_at = "2026-01-02T00:00:00Z"

        [metadata]
        custom_key = "value"

        [[notes]]
        author = "alice"
        author_email = "alice@example.com"
        body = "A note on this task"
      "#;
      std::fs::write(source.join("tasks/zyxwvutsrqponmlkzyxwvutsrqponmlk.toml"), task_toml).unwrap();

      let (store, project_id, _db_tmp) = setup_db().await;
      let conn = store.connect().await.unwrap();
      let mut counts = MigrationCounts::default();
      let id_map = migrate_tasks(&conn, &project_id, &source, &mut counts, &mut Vec::new())
        .await
        .unwrap();

      assert_eq!(counts.tasks, 1);
      assert_eq!(counts.notes, 1);
      assert_eq!(id_map.len(), 1);

      let new_id = id_map.get("zyxwvutsrqponmlkzyxwvutsrqponmlk").unwrap();
      let task = repo::task::find_by_id(&conn, new_id.clone()).await.unwrap().unwrap();
      assert_eq!(task.title(), "Test task");
      assert_eq!(task.description(), "A description");
      assert_eq!(task.priority(), Some(1));

      // Verify metadata
      assert_eq!(task.metadata()["custom_key"], "value");

      // Verify notes
      let notes = repo::note::for_entity(&conn, EntityType::Task, new_id).await.unwrap();
      assert_eq!(notes.len(), 1);
      assert_eq!(notes[0].body(), "A note on this task");
    }
  }

  mod parse_legacy_artifact {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_updated_at_when_archived_at_missing() {
      let content = "---\n\
        id: zyxwvutsrqponmlkzyxwvutsrqponmlk\n\
        title: Archived\n\
        created_at: \"2026-01-01T00:00:00Z\"\n\
        updated_at: \"2026-01-02T00:00:00Z\"\n\
        ---\n\n\
        Old.\n";
      let artifact = parse_legacy_artifact(content, true).unwrap();

      assert_eq!(artifact.archived_at, Some("2026-01-02T00:00:00Z".to_string()));
    }

    #[test]
    fn it_parses_artifact_with_body() {
      let content = "---\n\
        id: zyxwvutsrqponmlkzyxwvutsrqponmlk\n\
        title: Test\n\
        created_at: \"2026-01-01T00:00:00Z\"\n\
        updated_at: \"2026-01-01T00:00:00Z\"\n\
        ---\n\n\
        Body content here.\n";
      let artifact = parse_legacy_artifact(content, false).unwrap();

      assert_eq!(artifact.title, "Test");
      assert_eq!(artifact.body, "Body content here.\n");
      assert_eq!(artifact.archived_at, None);
    }

    #[test]
    fn it_parses_artifact_with_kind() {
      let content = "---\n\
        id: zyxwvutsrqponmlkzyxwvutsrqponmlk\n\
        title: ADR\n\
        type: adr\n\
        created_at: \"2026-01-01T00:00:00Z\"\n\
        updated_at: \"2026-01-01T00:00:00Z\"\n\
        ---\n\n\
        Decision.\n";
      let artifact = parse_legacy_artifact(content, false).unwrap();

      assert_eq!(artifact.kind.as_deref(), Some("adr"));
    }

    #[test]
    fn it_parses_artifact_with_tags() {
      let content = "---\n\
        id: zyxwvutsrqponmlkzyxwvutsrqponmlk\n\
        title: Tagged\n\
        tags:\n  - design\n  - adr\n\
        created_at: \"2026-01-01T00:00:00Z\"\n\
        updated_at: \"2026-01-01T00:00:00Z\"\n\
        ---\n\n\
        Content.\n";
      let artifact = parse_legacy_artifact(content, false).unwrap();

      assert_eq!(artifact.tags, vec!["design", "adr"]);
    }

    #[test]
    fn it_sets_archived_at_for_archived_artifacts() {
      let content = "---\n\
        id: zyxwvutsrqponmlkzyxwvutsrqponmlk\n\
        title: Archived\n\
        created_at: \"2026-01-01T00:00:00Z\"\n\
        updated_at: \"2026-01-02T00:00:00Z\"\n\
        archived_at: \"2026-01-03T00:00:00Z\"\n\
        ---\n\n\
        Old.\n";
      let artifact = parse_legacy_artifact(content, true).unwrap();

      assert_eq!(artifact.archived_at, Some("2026-01-03T00:00:00Z".to_string()));
    }
  }

  mod parse_legacy_iteration {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_iteration_with_tasks() {
      let toml_str = r#"
        id = "zyxwvutsrqponmlkzyxwvutsrqponmlk"
        title = "Sprint 1"
        description = "First sprint"
        status = "active"
        tags = ["sprint"]
        tasks = ["tasks/kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk"]
        created_at = "2026-01-01T00:00:00Z"
        updated_at = "2026-01-02T00:00:00Z"
      "#;
      let iteration: LegacyIteration = toml::from_str(toml_str).unwrap();

      assert_eq!(iteration.tasks.unwrap(), vec!["tasks/kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk"]);
      assert_eq!(iteration.tags.unwrap(), vec!["sprint"]);
    }

    #[test]
    fn it_parses_minimal_iteration() {
      let toml_str = r#"
        id = "zyxwvutsrqponmlkzyxwvutsrqponmlk"
        title = "Sprint"
        status = "active"
        created_at = "2026-01-01T00:00:00Z"
        updated_at = "2026-01-01T00:00:00Z"
      "#;
      let iteration: LegacyIteration = toml::from_str(toml_str).unwrap();

      assert_eq!(iteration.title, "Sprint");
      assert_eq!(iteration.description, None);
      assert!(iteration.tasks.unwrap_or_default().is_empty());
      assert!(iteration.tags.unwrap_or_default().is_empty());
    }
  }

  mod parse_legacy_task {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_minimal_task() {
      let toml_str = r#"
        id = "zyxwvutsrqponmlkzyxwvutsrqponmlk"
        title = "Minimal"
        status = "open"
        created_at = "2026-01-01T00:00:00Z"
        updated_at = "2026-01-01T00:00:00Z"
      "#;
      let task: LegacyTask = toml::from_str(toml_str).unwrap();

      assert_eq!(task.title, "Minimal");
      assert_eq!(task.description, None);
      assert_eq!(task.assigned_to, None);
      assert_eq!(task.priority, None);
      assert!(task.tags.unwrap_or_default().is_empty());
    }

    #[test]
    fn it_parses_task_with_all_fields() {
      let toml_str = r#"
        id = "zyxwvutsrqponmlkzyxwvutsrqponmlk"
        title = "Full task"
        description = "A description"
        status = "done"
        priority = 1
        assigned_to = "agent-1"
        tags = ["bug", "urgent"]
        created_at = "2026-01-01T00:00:00Z"
        updated_at = "2026-01-02T00:00:00Z"
        resolved_at = "2026-01-02T00:00:00Z"

        [[notes]]
        author = "alice"
        body = "A note"

        [[links]]
        ref = "tasks/kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk"
        rel = "blocked-by"
      "#;
      let task: LegacyTask = toml::from_str(toml_str).unwrap();

      assert_eq!(task.title, "Full task");
      assert_eq!(task.description.as_deref(), Some("A description"));
      assert_eq!(task.assigned_to.as_deref(), Some("agent-1"));
      assert_eq!(task.priority, Some(1));
      assert_eq!(task.tags.unwrap(), vec!["bug", "urgent"]);
      assert_eq!(task.resolved_at, Some("2026-01-02T00:00:00Z".to_string()));
      assert_eq!(task.notes.as_ref().unwrap().len(), 1);
      assert_eq!(task.links.as_ref().unwrap().len(), 1);
    }
  }

  mod parse_yaml_frontmatter {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_list_values() {
      let yaml = "tags:\n  - bug\n  - urgent";
      let fields = parse_yaml_frontmatter(yaml);
      let tags = parse_yaml_list_field(&fields, "tags");

      assert_eq!(tags, vec!["bug", "urgent"]);
    }

    #[test]
    fn it_parses_simple_key_value_pairs() {
      let yaml = "id: zyxwvutsrqponmlkzyxwvutsrqponmlk\ntitle: Test";
      let fields = parse_yaml_frontmatter(yaml);

      assert_eq!(fields.get("id").unwrap(), "zyxwvutsrqponmlkzyxwvutsrqponmlk");
      assert_eq!(fields.get("title").unwrap(), "Test");
    }

    #[test]
    fn it_strips_quotes() {
      let yaml = "created_at: \"2026-01-01T00:00:00Z\"";
      let fields = parse_yaml_frontmatter(yaml);

      assert_eq!(fields.get("created_at").unwrap(), "2026-01-01T00:00:00Z");
    }
  }

  mod toml_table_to_json_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_converts_toml_table_to_json() {
      let mut table = toml::Table::new();
      table.insert("key".to_string(), toml::Value::String("value".to_string()));
      let json = toml_table_to_json(&table).unwrap();
      assert_eq!(json["key"], "value");
    }

    #[test]
    fn it_returns_none_for_empty_table() {
      let table = toml::Table::new();
      assert_eq!(toml_table_to_json(&table), None);
    }
  }
}
