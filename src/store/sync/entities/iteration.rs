//! Per-entity sync adapter for iterations and their notes.
//!
//! - Iteration definitions live at `iteration/<id>.yaml` with their tags and
//!   `phases:` membership embedded at the top level (per ADR-0016 §3 — the
//!   iteration owns the phase ordering).
//! - Iteration notes are individual files at `iteration/notes/<note_id>.yaml`.

use std::{
  collections::{BTreeMap, HashSet},
  path::Path,
};

use chrono::{DateTime, Utc};
use libsql::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::store::{
  model::primitives::Id,
  sync::{Error, paths, yaml},
};

/// Upsert SQL for the `iterations` table.
const UPSERT_ITERATION_SQL: &str = "INSERT INTO iterations \
  (id, project_id, completed_at, created_at, description, metadata, status, title, updated_at) \
  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
  ON CONFLICT(id) DO UPDATE SET \
    completed_at = ?3, description = ?5, metadata = ?6, status = ?7, title = ?8, updated_at = ?9";

/// On-disk wrapper for `.gest/iteration/<id>.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct IterationFile {
  /// Tombstone instant; when set, import deletes the iteration and its phase rows.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  deleted_at: Option<DateTime<Utc>>,
  /// Stable iteration identifier; also the file's name stem.
  id: Id,
  /// Short human-readable name of the iteration.
  title: String,
  /// Lifecycle status, as stored in the `status` column.
  status: String,
  /// Free-form description; omitted from the file when empty.
  #[serde(default, skip_serializing_if = "String::is_empty")]
  description: String,
  /// Labels of the tags attached to the iteration, sorted; omitted when empty.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  tags: Vec<String>,
  /// Tasks of the iteration grouped by phase, in phase order; omitted when empty.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  phases: Vec<PhaseGroup>,
  /// Arbitrary JSON metadata; omitted when null.
  #[serde(default, skip_serializing_if = "JsonValue::is_null")]
  metadata: JsonValue,
  /// When the iteration was created.
  created_at: DateTime<Utc>,
  /// When the iteration was last modified.
  updated_at: DateTime<Utc>,
  /// When the iteration was completed, if it has been.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  completed_at: Option<DateTime<Utc>>,
}

/// The tasks of one phase of an iteration, as written under `phases:` in its file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PhaseGroup {
  /// Phase number within the iteration, from the `phase` column of `iteration_tasks`.
  phase: u32,
  /// Ids of the tasks in this phase, sorted by id on export.
  tasks: Vec<Id>,
}

/// On-disk wrapper for `.gest/iteration/notes/<note_id>.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct IterationNoteFile {
  /// Tombstone instant; when set, import deletes the note row instead of upserting it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  deleted_at: Option<DateTime<Utc>>,
  /// Stable note identifier; also the file's name stem.
  id: Id,
  /// The iteration the note belongs to.
  entity_id: Id,
  /// The author of the note, if one was recorded.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  author_id: Option<Id>,
  /// The note's text.
  body: String,
  /// When the note was created.
  created_at: DateTime<Utc>,
  /// When the note was last edited.
  updated_at: DateTime<Utc>,
}

/// Import every iteration and iteration-note file into SQLite.
///
/// Each iteration file is upserted with its tags and phases. A tombstone file deletes the iteration and its
/// `iteration_tasks` rows, and a note tombstone deletes the note.
///
/// # Arguments
///
/// * `conn` - the open database connection
/// * `project_id` - the project that new iteration rows belong to
/// * `gest_dir` - the project's `.gest/` directory
///
/// # Returns
///
/// `()` once every file is imported, or the first read, parse or database error.
pub async fn read_all(conn: &Connection, project_id: &Id, gest_dir: &Path) -> Result<(), Error> {
  let iteration_dir = gest_dir.join(paths::ITERATION_DIR);
  let notes_dir = iteration_dir.join(paths::NOTES_DIR);

  for path in yaml::walk_files(&iteration_dir, "yaml")? {
    if path.starts_with(&notes_dir) {
      continue;
    }
    let Some(file): Option<IterationFile> = yaml::read(&path)? else {
      continue;
    };
    if file.deleted_at.is_some() {
      log::info!("sync import: tombstone for iteration {}", file.id.short());
      conn
        .execute(
          "DELETE FROM iteration_tasks WHERE iteration_id = ?1",
          [file.id.to_string()],
        )
        .await?;
      conn
        .execute("DELETE FROM iterations WHERE id = ?1", [file.id.to_string()])
        .await?;
      continue;
    }
    upsert_iteration(conn, project_id, &file).await?;
    sync_iteration_tags(conn, &file).await?;
    sync_phases(conn, &file).await?;
  }

  for path in yaml::walk_files(&notes_dir, "yaml")? {
    let Some(file): Option<IterationNoteFile> = yaml::read(&path)? else {
      continue;
    };
    if file.deleted_at.is_some() {
      log::info!("sync import: tombstone for iteration note {}", file.id.short());
      conn
        .execute("DELETE FROM notes WHERE id = ?1", [file.id.to_string()])
        .await?;
      continue;
    }
    upsert_note(conn, &file).await?;
  }

  Ok(())
}

/// Export every iteration and iteration-note row to per-entity files.
///
/// Files for iterations and notes that no longer exist are removed afterwards, except tombstone files.
///
/// # Arguments
///
/// * `conn` - the open database connection
/// * `project_id` - the project whose iterations are exported, and whose `sync_digests` cache records the writes
/// * `gest_dir` - the project's `.gest/` directory
///
/// # Returns
///
/// `()` once every file is written and orphans are cleaned up, or the first database, parse or I/O error.
pub async fn write_all(conn: &Connection, project_id: &Id, gest_dir: &Path) -> Result<(), Error> {
  let mut alive_iterations: HashSet<String> = HashSet::new();
  let mut alive_notes: HashSet<String> = HashSet::new();

  let mut rows = conn
    .query(
      "SELECT id, completed_at, created_at, description, metadata, status, title, updated_at \
        FROM iterations WHERE project_id = ?1 ORDER BY id",
      [project_id.to_string()],
    )
    .await?;
  while let Some(row) = rows.next().await? {
    let id_str: String = row.get(0)?;
    let completed_at: Option<String> = row.get(1).ok();
    let created_at: String = row.get(2)?;
    let description: String = row.get(3)?;
    let metadata_str: String = row.get(4).unwrap_or_else(|_| "{}".to_string());
    let status: String = row.get(5)?;
    let title: String = row.get(6)?;
    let updated_at: String = row.get(7)?;

    let id: Id = id_str
      .parse()
      .map_err(|e: String| Error::Io(std::io::Error::other(e)))?;
    let metadata: JsonValue = serde_json::from_str(&metadata_str).unwrap_or(JsonValue::Null);
    let tags = load_iteration_tag_labels(conn, &id).await?;
    let phases = load_phases(conn, &id).await?;

    let file = IterationFile {
      deleted_at: None,
      id: id.clone(),
      title,
      status,
      description,
      tags,
      phases,
      metadata,
      created_at: parse_dt(&created_at)?,
      updated_at: parse_dt(&updated_at)?,
      completed_at: completed_at.as_deref().map(parse_dt).transpose()?,
    };
    let path = paths::iteration_path(gest_dir, &id);
    yaml::write_cached(conn, project_id, gest_dir, &path, &file).await?;
    alive_iterations.insert(id.to_string());
  }

  // Iteration notes
  let mut rows = conn
    .query(
      "SELECT n.id, n.entity_id, n.author_id, n.body, n.created_at, n.updated_at \
        FROM notes n \
        WHERE n.entity_type = 'iteration' AND n.entity_id IN (SELECT id FROM iterations WHERE project_id = ?1) \
        ORDER BY n.id",
      [project_id.to_string()],
    )
    .await?;
  while let Some(row) = rows.next().await? {
    let id_str: String = row.get(0)?;
    let entity_id_str: String = row.get(1)?;
    let author_id: Option<String> = row.get(2).ok();
    let body: String = row.get(3)?;
    let created_at: String = row.get(4)?;
    let updated_at: String = row.get(5)?;

    let id: Id = id_str
      .parse()
      .map_err(|e: String| Error::Io(std::io::Error::other(e)))?;
    let entity_id: Id = entity_id_str
      .parse()
      .map_err(|e: String| Error::Io(std::io::Error::other(e)))?;
    let author_id = match author_id {
      Some(s) => Some(s.parse().map_err(|e: String| Error::Io(std::io::Error::other(e)))?),
      None => None,
    };
    let file = IterationNoteFile {
      deleted_at: None,
      id: id.clone(),
      entity_id,
      author_id,
      body,
      created_at: parse_dt(&created_at)?,
      updated_at: parse_dt(&updated_at)?,
    };
    let path = paths::iteration_note_path(gest_dir, &id);
    yaml::write_cached(conn, project_id, gest_dir, &path, &file).await?;
    alive_notes.insert(id.to_string());
  }

  // Clean up orphaned iteration and iteration-note files. Tombstoned files
  // (YAML with a `deleted_at` key) are preserved so downstream clones can
  // pick up the deletion on their next import.
  let iteration_dir = gest_dir.join(paths::ITERATION_DIR);
  let notes_dir = iteration_dir.join(paths::NOTES_DIR);
  yaml::cleanup_orphans(conn, project_id, gest_dir, &notes_dir, "yaml", &alive_notes).await?;
  for entry in std::fs::read_dir(&iteration_dir).into_iter().flatten().flatten() {
    let path = entry.path();
    if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
      let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
      if !alive_iterations.contains(stem) && !yaml::is_tombstoned_yaml_file(&path) {
        let relative = paths::relative(gest_dir, &path).unwrap_or_default();
        std::fs::remove_file(&path)?;
        conn
          .execute(
            "DELETE FROM sync_digests WHERE relative_path = ?1 AND project_id = ?2",
            [relative, project_id.to_string()],
          )
          .await?;
      }
    }
  }

  Ok(())
}

/// Read the labels of the tags attached to an iteration.
///
/// # Arguments
///
/// * `conn` - the open database connection
/// * `iteration_id` - the iteration whose tags are read
///
/// # Returns
///
/// The labels sorted alphabetically, or the database error.
async fn load_iteration_tag_labels(conn: &Connection, iteration_id: &Id) -> Result<Vec<String>, Error> {
  let mut rows = conn
    .query(
      "SELECT t.label FROM entity_tags et JOIN tags t ON t.id = et.tag_id \
        WHERE et.entity_type = 'iteration' AND et.entity_id = ?1 ORDER BY t.label",
      [iteration_id.to_string()],
    )
    .await?;
  let mut labels = Vec::new();
  while let Some(row) = rows.next().await? {
    labels.push(row.get::<String>(0)?);
  }
  Ok(labels)
}

/// Read an iteration's tasks from `iteration_tasks`, grouped by phase.
///
/// # Arguments
///
/// * `conn` - the open database connection
/// * `iteration_id` - the iteration whose tasks are read
///
/// # Returns
///
/// One group per phase in ascending phase order, each with its task ids sorted, or a database or id-parse
/// error.
async fn load_phases(conn: &Connection, iteration_id: &Id) -> Result<Vec<PhaseGroup>, Error> {
  let mut rows = conn
    .query(
      "SELECT phase, task_id FROM iteration_tasks WHERE iteration_id = ?1 ORDER BY phase, task_id",
      [iteration_id.to_string()],
    )
    .await?;
  let mut grouped: BTreeMap<u32, Vec<Id>> = BTreeMap::new();
  while let Some(row) = rows.next().await? {
    let phase: i64 = row.get(0)?;
    let task_id_str: String = row.get(1)?;
    let task_id: Id = task_id_str
      .parse()
      .map_err(|e: String| Error::Io(std::io::Error::other(e)))?;
    grouped.entry(phase as u32).or_default().push(task_id);
  }
  Ok(
    grouped
      .into_iter()
      .map(|(phase, tasks)| PhaseGroup {
        phase,
        tasks,
      })
      .collect(),
  )
}

/// Parse an RFC 3339 timestamp from a database column.
///
/// # Arguments
///
/// * `s` - the timestamp text
///
/// # Returns
///
/// The instant in UTC, or an `Error::Io` holding the parse error.
fn parse_dt(s: &str) -> Result<DateTime<Utc>, Error> {
  DateTime::parse_from_rfc3339(s)
    .map(|dt| dt.with_timezone(&Utc))
    .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))
}

/// Replace an iteration's tag attachments with the labels in its file.
///
/// A label with no tag row gets a new tag.
///
/// # Arguments
///
/// * `conn` - the open database connection
/// * `file` - the parsed iteration file
///
/// # Returns
///
/// `()` on success, or the database error.
async fn sync_iteration_tags(conn: &Connection, file: &IterationFile) -> Result<(), Error> {
  conn
    .execute(
      "DELETE FROM entity_tags WHERE entity_type = 'iteration' AND entity_id = ?1",
      [file.id.to_string()],
    )
    .await?;
  for label in &file.tags {
    let mut rows = conn
      .query("SELECT id FROM tags WHERE label = ?1", [label.clone()])
      .await?;
    let tag_id: String = match rows.next().await? {
      Some(row) => row.get(0)?,
      None => {
        let new_id = Id::new();
        conn
          .execute(
            "INSERT INTO tags (id, label) VALUES (?1, ?2)",
            [new_id.to_string(), label.clone()],
          )
          .await?;
        new_id.to_string()
      }
    };
    conn
      .execute(
        "INSERT OR IGNORE INTO entity_tags (entity_type, entity_id, tag_id) VALUES ('iteration', ?1, ?2)",
        [file.id.to_string(), tag_id],
      )
      .await?;
  }
  Ok(())
}

/// Replace an iteration's `iteration_tasks` rows with the phases in its file.
///
/// # Arguments
///
/// * `conn` - the open database connection
/// * `file` - the parsed iteration file
///
/// # Returns
///
/// `()` on success, or the database error.
async fn sync_phases(conn: &Connection, file: &IterationFile) -> Result<(), Error> {
  conn
    .execute(
      "DELETE FROM iteration_tasks WHERE iteration_id = ?1",
      [file.id.to_string()],
    )
    .await?;
  for group in &file.phases {
    for task_id in &group.tasks {
      conn
        .execute(
          "INSERT OR IGNORE INTO iteration_tasks (iteration_id, task_id, phase) VALUES (?1, ?2, ?3)",
          libsql::params![file.id.to_string(), task_id.to_string(), group.phase as i64],
        )
        .await?;
    }
  }
  Ok(())
}

/// Insert an iteration from its file, or update the existing row with the same id.
///
/// On conflict `project_id` and `created_at` keep their stored values.
///
/// # Arguments
///
/// * `conn` - the open database connection
/// * `project_id` - the project a new row belongs to
/// * `file` - the parsed iteration file
///
/// # Returns
///
/// `()` on success, or the database error.
async fn upsert_iteration(conn: &Connection, project_id: &Id, file: &IterationFile) -> Result<(), Error> {
  let metadata_str = serde_json::to_string(&file.metadata).unwrap_or_else(|_| "{}".to_string());
  conn
    .execute(
      UPSERT_ITERATION_SQL,
      libsql::params![
        file.id.to_string(),
        project_id.to_string(),
        file.completed_at.map(|d| d.to_rfc3339()),
        file.created_at.to_rfc3339(),
        file.description.clone(),
        metadata_str,
        file.status.clone(),
        file.title.clone(),
        file.updated_at.to_rfc3339(),
      ],
    )
    .await?;
  Ok(())
}

/// Insert an iteration note from its file, or update the body and `updated_at` of the existing row.
///
/// # Arguments
///
/// * `conn` - the open database connection
/// * `file` - the parsed note file
///
/// # Returns
///
/// `()` on success, or the database error.
async fn upsert_note(conn: &Connection, file: &IterationNoteFile) -> Result<(), Error> {
  conn
    .execute(
      "INSERT INTO notes (id, entity_id, entity_type, author_id, body, created_at, updated_at) \
        VALUES (?1, ?2, 'iteration', ?3, ?4, ?5, ?6) \
        ON CONFLICT(id) DO UPDATE SET body = ?4, updated_at = ?6",
      libsql::params![
        file.id.to_string(),
        file.entity_id.to_string(),
        file.author_id.as_ref().map(|i| i.to_string()),
        file.body.clone(),
        file.created_at.to_rfc3339(),
        file.updated_at.to_rfc3339(),
      ],
    )
    .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use tempfile::TempDir;

  use super::*;
  use crate::store;

  async fn setup() -> (std::sync::Arc<store::Db>, TempDir, Id, PathBuf) {
    let (db, _tmp_db) = store::open_temp().await.unwrap();
    let conn = db.connect().await.unwrap();
    let project_root = TempDir::new().unwrap();
    let gest_dir = project_root.path().join(".gest");
    std::fs::create_dir_all(&gest_dir).unwrap();
    let pid = Id::new();
    conn
      .execute(
        "INSERT INTO projects (id, root, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        [
          pid.to_string(),
          project_root.path().to_string_lossy().into_owned(),
          "2026-04-08T00:00:00Z".to_string(),
          "2026-04-08T00:00:00Z".to_string(),
        ],
      )
      .await
      .unwrap();
    std::mem::forget(_tmp_db);
    (db, project_root, pid, gest_dir)
  }

  async fn insert_iteration(conn: &libsql::Connection, project_id: &Id, title: &str) -> Id {
    let id = Id::new();
    conn
      .execute(
        "INSERT INTO iterations (id, project_id, title, status, created_at, updated_at) \
          VALUES (?1, ?2, ?3, 'active', ?4, ?4)",
        libsql::params![
          id.to_string(),
          project_id.to_string(),
          title.to_string(),
          "2026-04-08T00:00:00Z".to_string(),
        ],
      )
      .await
      .unwrap();
    id
  }

  async fn insert_task(conn: &libsql::Connection, project_id: &Id) -> Id {
    let id = Id::new();
    conn
      .execute(
        "INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) \
          VALUES (?1, ?2, 'a task', 'open', ?3, ?3)",
        libsql::params![
          id.to_string(),
          project_id.to_string(),
          "2026-04-08T00:00:00Z".to_string(),
        ],
      )
      .await
      .unwrap();
    id
  }

  mod read_all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_roundtrips_iteration_with_phases_through_disk() {
      let (db, _root, pid, gest_dir) = setup().await;
      let conn = db.connect().await.unwrap();
      let iter_id = insert_iteration(&conn, &pid, "Sprint Roundtrip").await;
      let task_id = insert_task(&conn, &pid).await;
      conn
        .execute(
          "INSERT INTO iteration_tasks (iteration_id, task_id, phase) VALUES (?1, ?2, 3)",
          libsql::params![iter_id.to_string(), task_id.to_string()],
        )
        .await
        .unwrap();

      write_all(&conn, &pid, &gest_dir).await.unwrap();

      conn.execute("DELETE FROM iteration_tasks", ()).await.unwrap();
      conn.execute("DELETE FROM iterations", ()).await.unwrap();

      read_all(&conn, &pid, &gest_dir).await.unwrap();

      let mut rows = conn
        .query("SELECT title FROM iterations WHERE id = ?1", [iter_id.to_string()])
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      let title: String = row.get(0).unwrap();
      assert_eq!(title, "Sprint Roundtrip");

      let mut rows = conn
        .query(
          "SELECT phase, task_id FROM iteration_tasks WHERE iteration_id = ?1",
          [iter_id.to_string()],
        )
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      let phase: i64 = row.get(0).unwrap();
      let task_id_db: String = row.get(1).unwrap();
      assert_eq!(phase, 3);
      assert_eq!(task_id_db, task_id.to_string());
    }
  }

  mod write_all {
    use super::*;

    #[tokio::test]
    async fn it_writes_an_iteration_file_with_embedded_phases() {
      let (db, _root, pid, gest_dir) = setup().await;
      let conn = db.connect().await.unwrap();
      let iter_id = insert_iteration(&conn, &pid, "Sprint 1").await;
      let task_a = insert_task(&conn, &pid).await;
      let task_b = insert_task(&conn, &pid).await;
      conn
        .execute(
          "INSERT INTO iteration_tasks (iteration_id, task_id, phase) VALUES (?1, ?2, 1), (?1, ?3, 2)",
          libsql::params![iter_id.to_string(), task_a.to_string(), task_b.to_string()],
        )
        .await
        .unwrap();

      write_all(&conn, &pid, &gest_dir).await.unwrap();

      let path = paths::iteration_path(&gest_dir, &iter_id);
      assert!(path.exists());
      let raw = std::fs::read_to_string(&path).unwrap();
      assert!(raw.contains("title: Sprint 1"));
      assert!(raw.contains("phases:"));
      assert!(raw.contains("phase: 1"));
      assert!(raw.contains("phase: 2"));
      assert!(raw.contains(&task_a.to_string()));
      assert!(raw.contains(&task_b.to_string()));
    }
  }
}
