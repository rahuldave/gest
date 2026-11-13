//! Read-only selectors that enumerate entities eligible for purge.
//!
//! Each function returns a count plus the list of eligible IDs/paths, scoped to
//! either a single project or all projects. Functions are side-effect-free: no
//! writes, no file modifications. Suitable for use by `--dry-run`.

use std::path::{Path, PathBuf};

use libsql::Connection;

use crate::store::{Error, model::primitives::Id, sync::paths};

/// Scope for purge selectors: either a single project or all projects.
#[derive(Clone, Debug)]
pub enum Scope {
  /// Restrict to all projects.
  AllProjects,
  /// Restrict to a single project.
  Project(Id),
}

/// Terminal tasks grouped by status.
#[derive(Clone, Debug, Default)]
pub struct TerminalTasks {
  /// Number of tasks in the `cancelled` state.
  pub cancelled: usize,
  /// Number of tasks in the `done` state.
  pub done: usize,
  /// IDs of all terminal tasks.
  pub ids: Vec<Id>,
}

impl TerminalTasks {
  /// Total number of terminal tasks.
  ///
  /// # Returns
  ///
  /// The sum of [`cancelled`](Self::cancelled) and [`done`](Self::done).
  pub fn total(&self) -> usize {
    self.cancelled + self.done
  }
}

/// Terminal iterations grouped by status.
#[derive(Clone, Debug, Default)]
pub struct TerminalIterations {
  /// Number of iterations in the `cancelled` state.
  pub cancelled: usize,
  /// Number of iterations in the `completed` state.
  pub completed: usize,
  /// IDs of all terminal iterations.
  pub ids: Vec<Id>,
}

impl TerminalIterations {
  /// Total number of terminal iterations.
  ///
  /// # Returns
  ///
  /// The sum of [`cancelled`](Self::cancelled) and [`completed`](Self::completed).
  pub fn total(&self) -> usize {
    self.cancelled + self.completed
  }
}

/// Archived artifacts.
#[derive(Clone, Debug, Default)]
pub struct ArchivedArtifacts {
  /// Number of archived artifacts.
  pub count: usize,
  /// IDs of all archived artifacts.
  pub ids: Vec<Id>,
}

/// Archived projects.
#[derive(Clone, Debug, Default)]
pub struct ArchivedProjects {
  /// Number of archived projects.
  pub count: usize,
  /// IDs of all archived projects.
  pub ids: Vec<Id>,
}

/// A relationship row whose source or target no longer resolves to a live entity.
#[derive(Clone, Debug, Default)]
pub struct DanglingRelationships {
  /// Number of dangling relationship rows.
  pub count: usize,
  /// IDs of all dangling relationship rows.
  pub ids: Vec<Id>,
}

/// On-disk tombstone files with no matching DB row.
#[derive(Clone, Debug, Default)]
pub struct OrphanTombstones {
  /// Number of orphan tombstone files.
  pub count: usize,
  /// Filesystem paths of all orphan tombstone files.
  pub paths: Vec<PathBuf>,
}

/// Return tasks in terminal status (`done` or `cancelled`).
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `scope` - one project, or all projects
///
/// # Returns
///
/// The counts by status and the ids of the tasks. It is [`Error::InvalidValue`] when a stored id does not parse, or
/// a database error.
pub async fn terminal_tasks(conn: &Connection, scope: &Scope) -> Result<TerminalTasks, Error> {
  log::debug!("repo::purge::terminal_tasks");
  let (sql, params) = match scope {
    Scope::AllProjects => (
      "SELECT id, status FROM tasks WHERE status IN ('done', 'cancelled')".to_string(),
      vec![],
    ),
    Scope::Project(pid) => (
      "SELECT id, status FROM tasks WHERE project_id = ?1 AND status IN ('done', 'cancelled')".to_string(),
      vec![pid.to_string()],
    ),
  };

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  let mut result = TerminalTasks::default();
  while let Some(row) = rows.next().await? {
    let id: String = row.get(0)?;
    let status: String = row.get(1)?;
    let id: Id = id.parse().map_err(Error::InvalidValue)?;
    match status.as_str() {
      "cancelled" => result.cancelled += 1,
      "done" => result.done += 1,
      _ => {}
    }
    result.ids.push(id);
  }
  Ok(result)
}

/// Return iterations in terminal status (`completed` or `cancelled`).
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `scope` - one project, or all projects
///
/// # Returns
///
/// The counts by status and the ids of the iterations. It is [`Error::InvalidValue`] when a stored id does not
/// parse, or a database error.
pub async fn terminal_iterations(conn: &Connection, scope: &Scope) -> Result<TerminalIterations, Error> {
  log::debug!("repo::purge::terminal_iterations");
  let (sql, params) = match scope {
    Scope::AllProjects => (
      "SELECT id, status FROM iterations WHERE status IN ('completed', 'cancelled')".to_string(),
      vec![],
    ),
    Scope::Project(pid) => (
      "SELECT id, status FROM iterations WHERE project_id = ?1 AND status IN ('completed', 'cancelled')".to_string(),
      vec![pid.to_string()],
    ),
  };

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  let mut result = TerminalIterations::default();
  while let Some(row) = rows.next().await? {
    let id: String = row.get(0)?;
    let status: String = row.get(1)?;
    let id: Id = id.parse().map_err(Error::InvalidValue)?;
    match status.as_str() {
      "cancelled" => result.cancelled += 1,
      "completed" => result.completed += 1,
      _ => {}
    }
    result.ids.push(id);
  }
  Ok(result)
}

/// Return artifacts with `archived_at IS NOT NULL`.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `scope` - one project, or all projects
///
/// # Returns
///
/// The count and ids of the archived artifacts. It is [`Error::InvalidValue`] when a stored id does not parse, or a
/// database error.
pub async fn archived_artifacts(conn: &Connection, scope: &Scope) -> Result<ArchivedArtifacts, Error> {
  log::debug!("repo::purge::archived_artifacts");
  let (sql, params) = match scope {
    Scope::AllProjects => (
      "SELECT id FROM artifacts WHERE archived_at IS NOT NULL".to_string(),
      vec![],
    ),
    Scope::Project(pid) => (
      "SELECT id FROM artifacts WHERE project_id = ?1 AND archived_at IS NOT NULL".to_string(),
      vec![pid.to_string()],
    ),
  };

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  let mut result = ArchivedArtifacts::default();
  while let Some(row) = rows.next().await? {
    let id: String = row.get(0)?;
    let id: Id = id.parse().map_err(Error::InvalidValue)?;
    result.ids.push(id);
    result.count += 1;
  }
  Ok(result)
}

/// Return projects with `archived_at IS NOT NULL`.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `scope` - all projects, or one project, which is then the only one that can match
///
/// # Returns
///
/// The count and ids of the archived projects. It is [`Error::InvalidValue`] when a stored id does not parse, or a
/// database error.
pub async fn archived_projects(conn: &Connection, scope: &Scope) -> Result<ArchivedProjects, Error> {
  log::debug!("repo::purge::archived_projects");
  let (sql, params) = match scope {
    Scope::AllProjects => (
      "SELECT id FROM projects WHERE archived_at IS NOT NULL".to_string(),
      vec![],
    ),
    Scope::Project(pid) => (
      "SELECT id FROM projects WHERE id = ?1 AND archived_at IS NOT NULL".to_string(),
      vec![pid.to_string()],
    ),
  };

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  let mut result = ArchivedProjects::default();
  while let Some(row) = rows.next().await? {
    let id: String = row.get(0)?;
    let id: Id = id.parse().map_err(Error::InvalidValue)?;
    result.ids.push(id);
    result.count += 1;
  }
  Ok(result)
}

/// Return relationship rows whose `source_id` or `target_id` no longer
/// resolves to a live entity (task, iteration or artifact) in the DB.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `scope` - all projects, or one project; for one project, only relationships with an endpoint in that project,
///   or with neither endpoint left anywhere, count
///
/// # Returns
///
/// The count and ids of the dangling relationships. It is [`Error::InvalidValue`] when a stored id does not parse,
/// or a database error.
pub async fn dangling_relationships(conn: &Connection, scope: &Scope) -> Result<DanglingRelationships, Error> {
  log::debug!("repo::purge::dangling_relationships");

  // A relationship is dangling if either endpoint no longer exists in its
  // respective table. We check the three tables a relationship can point at:
  // tasks, iterations and artifacts, the variants of EntityType. For Project
  // scope we limit to relationships that touch at least one entity owned by
  // the project.
  let sql = match scope {
    Scope::AllProjects => "SELECT r.id FROM relationships r \
        WHERE NOT EXISTS ( \
          SELECT 1 FROM tasks t WHERE t.id = r.source_id AND r.source_type = 'task' \
          UNION ALL SELECT 1 FROM iterations i WHERE i.id = r.source_id AND r.source_type = 'iteration' \
          UNION ALL SELECT 1 FROM artifacts a WHERE a.id = r.source_id AND r.source_type = 'artifact' \
        ) \
        OR NOT EXISTS ( \
          SELECT 1 FROM tasks t WHERE t.id = r.target_id AND r.target_type = 'task' \
          UNION ALL SELECT 1 FROM iterations i WHERE i.id = r.target_id AND r.target_type = 'iteration' \
          UNION ALL SELECT 1 FROM artifacts a WHERE a.id = r.target_id AND r.target_type = 'artifact' \
        )"
    .to_string(),
    Scope::Project(pid) => {
      format!(
        "SELECT r.id FROM relationships r \
          WHERE ( \
            NOT EXISTS ( \
              SELECT 1 FROM tasks t WHERE t.id = r.source_id AND r.source_type = 'task' \
              UNION ALL SELECT 1 FROM iterations i WHERE i.id = r.source_id AND r.source_type = 'iteration' \
              UNION ALL SELECT 1 FROM artifacts a WHERE a.id = r.source_id AND r.source_type = 'artifact' \
            ) \
            OR NOT EXISTS ( \
              SELECT 1 FROM tasks t WHERE t.id = r.target_id AND r.target_type = 'task' \
              UNION ALL SELECT 1 FROM iterations i WHERE i.id = r.target_id AND r.target_type = 'iteration' \
              UNION ALL SELECT 1 FROM artifacts a WHERE a.id = r.target_id AND r.target_type = 'artifact' \
            ) \
          ) \
          AND ( \
            EXISTS (SELECT 1 FROM tasks t WHERE t.id = r.source_id AND t.project_id = '{pid}') \
            OR EXISTS (SELECT 1 FROM tasks t WHERE t.id = r.target_id AND t.project_id = '{pid}') \
            OR EXISTS (SELECT 1 FROM iterations i WHERE i.id = r.source_id AND i.project_id = '{pid}') \
            OR EXISTS (SELECT 1 FROM iterations i WHERE i.id = r.target_id AND i.project_id = '{pid}') \
            OR EXISTS (SELECT 1 FROM artifacts a WHERE a.id = r.source_id AND a.project_id = '{pid}') \
            OR EXISTS (SELECT 1 FROM artifacts a WHERE a.id = r.target_id AND a.project_id = '{pid}') \
            OR NOT EXISTS ( \
              SELECT 1 FROM tasks t WHERE t.id = r.source_id \
              UNION ALL SELECT 1 FROM iterations i WHERE i.id = r.source_id \
              UNION ALL SELECT 1 FROM artifacts a WHERE a.id = r.source_id \
              UNION ALL SELECT 1 FROM tasks t WHERE t.id = r.target_id \
              UNION ALL SELECT 1 FROM iterations i WHERE i.id = r.target_id \
              UNION ALL SELECT 1 FROM artifacts a WHERE a.id = r.target_id \
            ) \
          )"
      )
    }
  };

  let mut rows = conn.query(&sql, ()).await?;
  let mut result = DanglingRelationships::default();
  while let Some(row) = rows.next().await? {
    let id: String = row.get(0)?;
    let id: Id = id.parse().map_err(Error::InvalidValue)?;
    result.ids.push(id);
    result.count += 1;
  }
  Ok(result)
}

/// Return on-disk tombstone files (files with a `deleted_at` field) that have
/// no matching DB row.
///
/// This scans the `.gest/` directories for YAML/markdown files that contain a
/// `deleted_at` field, then checks whether the corresponding entity still
/// exists in the database. Files that have been tombstoned but whose entity
/// row has already been removed are considered orphans.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `scope` - all projects, or one project, which picks its entries of `gest_dirs`
/// * `gest_dirs` - each project's id with the path of its `.gest/` directory
///
/// # Returns
///
/// The count and paths of the orphan tombstone files, or an I/O or database error.
pub async fn orphan_tombstones(
  conn: &Connection,
  scope: &Scope,
  gest_dirs: &[(Id, PathBuf)],
) -> Result<OrphanTombstones, Error> {
  log::debug!("repo::purge::orphan_tombstones");
  let mut result = OrphanTombstones::default();

  let dirs: Vec<&(Id, PathBuf)> = match scope {
    Scope::AllProjects => gest_dirs.iter().collect(),
    Scope::Project(pid) => gest_dirs.iter().filter(|(id, _)| id == pid).collect(),
  };

  for (_project_id, gest_dir) in dirs {
    collect_orphan_tombstones_in_dir(conn, gest_dir, &mut result).await?;
  }

  Ok(result)
}

/// Add the orphan tombstones of one `.gest/` directory's tasks, iterations and artifacts to `result`.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `gest_dir` - the project's `.gest/` directory
/// * `result` - the tally that each orphan is added to
///
/// # Returns
///
/// `Ok(())` once the three folders are scanned, or an I/O or database error.
async fn collect_orphan_tombstones_in_dir(
  conn: &Connection,
  gest_dir: &Path,
  result: &mut OrphanTombstones,
) -> Result<(), Error> {
  // Check task tombstones
  scan_yaml_tombstones(conn, &gest_dir.join(paths::TASK_DIR), "tasks", result).await?;
  // Check iteration tombstones
  scan_yaml_tombstones(conn, &gest_dir.join(paths::ITERATION_DIR), "iterations", result).await?;
  // Check artifact tombstones (markdown with frontmatter)
  scan_artifact_tombstones(conn, &gest_dir.join(paths::ARTIFACT_DIR), result).await?;
  Ok(())
}

/// Add each `.yaml` file in `dir` that mentions `deleted_at` and whose id has no row in `table` to `result`.
///
/// A missing `dir` adds nothing. A file whose stem is not a valid id is skipped.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `dir` - the entity folder to scan, not recursively
/// * `table` - the table to look the id up in, such as `tasks`
/// * `result` - the tally that each orphan is added to
///
/// # Returns
///
/// `Ok(())` once the folder is scanned, or an I/O or database error.
async fn scan_yaml_tombstones(
  conn: &Connection,
  dir: &Path,
  table: &str,
  result: &mut OrphanTombstones,
) -> Result<(), Error> {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
    Err(e) => return Err(Error::Io(e)),
  };

  for entry in entries {
    let entry = entry?;
    let path = entry.path();
    if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
      continue;
    }
    // Skip notes subdirectory
    if path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) == Some(paths::NOTES_DIR) {
      continue;
    }
    let raw = std::fs::read_to_string(&path)?;
    if !raw.contains("deleted_at") {
      continue;
    }
    // Extract ID from filename (strip .yaml)
    let id_str = match path.file_stem().and_then(|s| s.to_str()) {
      Some(s) => s,
      None => continue,
    };
    let id: Id = match id_str.parse() {
      Ok(id) => id,
      Err(_) => continue,
    };
    if !entity_exists_in_table(conn, table, &id).await? {
      result.paths.push(path);
      result.count += 1;
    }
  }
  Ok(())
}

/// Add each `.md` artifact file in `dir` that mentions `deleted_at` and has no `artifacts` row to `result`.
///
/// A missing `dir` adds nothing. A file whose stem is not a valid id is skipped.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `dir` - the artifact folder to scan, not recursively
/// * `result` - the tally that each orphan is added to
///
/// # Returns
///
/// `Ok(())` once the folder is scanned, or an I/O or database error.
async fn scan_artifact_tombstones(conn: &Connection, dir: &Path, result: &mut OrphanTombstones) -> Result<(), Error> {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
    Err(e) => return Err(Error::Io(e)),
  };

  for entry in entries {
    let entry = entry?;
    let path = entry.path();
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
      continue;
    }
    let raw = std::fs::read_to_string(&path)?;
    if !raw.contains("deleted_at") {
      continue;
    }
    let id_str = match path.file_stem().and_then(|s| s.to_str()) {
      Some(s) => s,
      None => continue,
    };
    let id: Id = match id_str.parse() {
      Ok(id) => id,
      Err(_) => continue,
    };
    if !entity_exists_in_table(conn, "artifacts", &id).await? {
      result.paths.push(path);
      result.count += 1;
    }
  }
  Ok(())
}

/// Check whether `table` has a row with this id.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `table` - the table name, put into the SQL as is
/// * `id` - the id to look for
///
/// # Returns
///
/// `true` when a row exists, or a database error.
async fn entity_exists_in_table(conn: &Connection, table: &str, id: &Id) -> Result<bool, Error> {
  let sql = format!("SELECT 1 FROM {table} WHERE id = ?1 LIMIT 1");
  let mut rows = conn.query(&sql, [id.to_string()]).await?;
  Ok(rows.next().await?.is_some())
}

#[cfg(test)]
mod tests {
  use std::{fs, sync::Arc};

  use tempfile::TempDir;

  use super::*;
  use crate::store::{
    self, Db,
    model::{
      Project,
      primitives::{EntityType, Id, RelationshipType},
    },
    repo::{artifact, iteration, relationship, task},
    sync::paths,
  };

  async fn setup() -> (Arc<Db>, Connection, TempDir, Id) {
    let (store, tmp) = store::open_temp().await.unwrap();
    let conn = store.connect().await.unwrap();
    let project = Project::new("/tmp/purge-test".into());
    conn
      .execute(
        "INSERT INTO projects (id, root, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        [
          project.id().to_string(),
          project.root().to_string_lossy().into_owned(),
          project.created_at().to_rfc3339(),
          project.updated_at().to_rfc3339(),
        ],
      )
      .await
      .unwrap();
    let project_id = project.id().clone();
    (store, conn, tmp, project_id)
  }

  async fn setup_two_projects() -> (Arc<Db>, Connection, TempDir, Id, Id) {
    let (store, tmp) = store::open_temp().await.unwrap();
    let conn = store.connect().await.unwrap();

    let p1 = Project::new("/tmp/purge-test-1".into());
    conn
      .execute(
        "INSERT INTO projects (id, root, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        [
          p1.id().to_string(),
          p1.root().to_string_lossy().into_owned(),
          p1.created_at().to_rfc3339(),
          p1.updated_at().to_rfc3339(),
        ],
      )
      .await
      .unwrap();

    let p2 = Project::new("/tmp/purge-test-2".into());
    conn
      .execute(
        "INSERT INTO projects (id, root, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        [
          p2.id().to_string(),
          p2.root().to_string_lossy().into_owned(),
          p2.created_at().to_rfc3339(),
          p2.updated_at().to_rfc3339(),
        ],
      )
      .await
      .unwrap();

    (store, conn, tmp, p1.id().clone(), p2.id().clone())
  }

  mod archived_artifacts_fn {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::artifact::New;

    #[tokio::test]
    async fn it_returns_archived_artifacts_for_project() {
      let (_store, conn, _tmp, pid) = setup().await;

      let active = artifact::create(
        &conn,
        &pid,
        &New {
          title: "Active".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let archived = artifact::create(
        &conn,
        &pid,
        &New {
          title: "Archived".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      conn
        .execute(
          "UPDATE artifacts SET archived_at = datetime('now') WHERE id = ?1",
          [archived.id().to_string()],
        )
        .await
        .unwrap();

      let result = archived_artifacts(&conn, &Scope::Project(pid)).await.unwrap();

      assert_eq!(result.count, 1);
      assert_eq!(result.ids.len(), 1);
      assert_eq!(&result.ids[0], archived.id());
      drop(active);
    }

    #[tokio::test]
    async fn it_returns_all_archived_artifacts_across_projects() {
      let (_store, conn, _tmp, pid1, pid2) = setup_two_projects().await;

      let a1 = artifact::create(
        &conn,
        &pid1,
        &New {
          title: "A1".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let a2 = artifact::create(
        &conn,
        &pid2,
        &New {
          title: "A2".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      conn
        .execute(
          "UPDATE artifacts SET archived_at = datetime('now') WHERE id = ?1",
          [a1.id().to_string()],
        )
        .await
        .unwrap();
      conn
        .execute(
          "UPDATE artifacts SET archived_at = datetime('now') WHERE id = ?1",
          [a2.id().to_string()],
        )
        .await
        .unwrap();

      let result = archived_artifacts(&conn, &Scope::AllProjects).await.unwrap();

      assert_eq!(result.count, 2);
    }

    #[tokio::test]
    async fn it_returns_empty_when_no_archived_artifacts() {
      let (_store, conn, _tmp, pid) = setup().await;

      artifact::create(
        &conn,
        &pid,
        &New {
          title: "Active".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let result = archived_artifacts(&conn, &Scope::Project(pid)).await.unwrap();

      assert_eq!(result.count, 0);
      assert!(result.ids.is_empty());
    }
  }

  mod archived_projects_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_archived_projects() {
      let (_store, conn, _tmp, pid1, _pid2) = setup_two_projects().await;

      conn
        .execute(
          "UPDATE projects SET archived_at = datetime('now') WHERE id = ?1",
          [pid1.to_string()],
        )
        .await
        .unwrap();

      let result = archived_projects(&conn, &Scope::AllProjects).await.unwrap();

      assert_eq!(result.count, 1);
      assert_eq!(result.ids[0], pid1);
    }

    #[tokio::test]
    async fn it_returns_single_archived_project_for_project_scope() {
      let (_store, conn, _tmp, pid1, _pid2) = setup_two_projects().await;

      conn
        .execute(
          "UPDATE projects SET archived_at = datetime('now') WHERE id = ?1",
          [pid1.to_string()],
        )
        .await
        .unwrap();

      let result = archived_projects(&conn, &Scope::Project(pid1.clone())).await.unwrap();

      assert_eq!(result.count, 1);
      assert_eq!(result.ids[0], pid1);
    }

    #[tokio::test]
    async fn it_returns_empty_when_no_archived_projects() {
      let (_store, conn, _tmp, _pid1, _pid2) = setup_two_projects().await;

      let result = archived_projects(&conn, &Scope::AllProjects).await.unwrap();

      assert_eq!(result.count, 0);
      assert!(result.ids.is_empty());
    }
  }

  mod dangling_relationships_fn {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::task::New;

    #[tokio::test]
    async fn it_finds_relationships_with_missing_target() {
      let (_store, conn, _tmp, pid) = setup().await;

      let t1 = task::create(
        &conn,
        &pid,
        &New {
          title: "T1".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let t2 = task::create(
        &conn,
        &pid,
        &New {
          title: "T2".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let rel = relationship::create(
        &conn,
        RelationshipType::Blocks,
        EntityType::Task,
        t1.id(),
        EntityType::Task,
        t2.id(),
      )
      .await
      .unwrap();

      // Delete target task, making the relationship dangling
      task::delete(&conn, t2.id()).await.unwrap();

      let result = dangling_relationships(&conn, &Scope::Project(pid)).await.unwrap();

      assert_eq!(result.count, 1);
      assert_eq!(result.ids[0], *rel.id());
    }

    #[tokio::test]
    async fn it_finds_relationships_with_missing_source() {
      let (_store, conn, _tmp, pid) = setup().await;

      let t1 = task::create(
        &conn,
        &pid,
        &New {
          title: "T1".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let t2 = task::create(
        &conn,
        &pid,
        &New {
          title: "T2".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let rel = relationship::create(
        &conn,
        RelationshipType::Blocks,
        EntityType::Task,
        t1.id(),
        EntityType::Task,
        t2.id(),
      )
      .await
      .unwrap();

      // Delete source task
      task::delete(&conn, t1.id()).await.unwrap();

      let result = dangling_relationships(&conn, &Scope::AllProjects).await.unwrap();

      assert_eq!(result.count, 1);
      assert_eq!(result.ids[0], *rel.id());
    }

    #[tokio::test]
    async fn it_returns_empty_when_all_relationships_valid() {
      let (_store, conn, _tmp, pid) = setup().await;

      let t1 = task::create(
        &conn,
        &pid,
        &New {
          title: "T1".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let t2 = task::create(
        &conn,
        &pid,
        &New {
          title: "T2".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      relationship::create(
        &conn,
        RelationshipType::Blocks,
        EntityType::Task,
        t1.id(),
        EntityType::Task,
        t2.id(),
      )
      .await
      .unwrap();

      let result = dangling_relationships(&conn, &Scope::Project(pid)).await.unwrap();

      assert_eq!(result.count, 0);
      assert!(result.ids.is_empty());
    }
  }

  mod orphan_tombstones_fn {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::task::New;

    #[tokio::test]
    async fn it_finds_tombstoned_yaml_files_with_no_db_row() {
      let (_store, conn, _tmp, pid) = setup().await;

      let gest_dir = tempfile::tempdir().unwrap();
      let task_dir = gest_dir.path().join(paths::TASK_DIR);
      fs::create_dir_all(&task_dir).unwrap();

      // Create a tombstoned task file for a non-existent entity
      let fake_id: Id = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk".parse().unwrap();
      let task_path = paths::task_path(gest_dir.path(), &fake_id);
      fs::write(
        &task_path,
        "id: kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\ntitle: Ghost\ndeleted_at: 2026-04-01T00:00:00Z\n",
      )
      .unwrap();

      let dirs = vec![(pid.clone(), gest_dir.path().to_path_buf())];
      let result = orphan_tombstones(&conn, &Scope::Project(pid), &dirs).await.unwrap();

      assert_eq!(result.count, 1);
      assert_eq!(result.paths[0], task_path);
    }

    #[tokio::test]
    async fn it_ignores_tombstoned_files_that_still_have_db_rows() {
      let (_store, conn, _tmp, pid) = setup().await;

      let t = task::create(
        &conn,
        &pid,
        &New {
          title: "Alive".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let gest_dir = tempfile::tempdir().unwrap();
      let task_dir = gest_dir.path().join(paths::TASK_DIR);
      fs::create_dir_all(&task_dir).unwrap();

      let task_path = paths::task_path(gest_dir.path(), t.id());
      fs::write(
        &task_path,
        format!("id: {}\ntitle: Alive\ndeleted_at: 2026-04-01T00:00:00Z\n", t.id()),
      )
      .unwrap();

      let dirs = vec![(pid.clone(), gest_dir.path().to_path_buf())];
      let result = orphan_tombstones(&conn, &Scope::Project(pid), &dirs).await.unwrap();

      assert_eq!(result.count, 0);
      assert!(result.paths.is_empty());
    }

    #[tokio::test]
    async fn it_ignores_non_tombstoned_files() {
      let (_store, conn, _tmp, pid) = setup().await;

      let gest_dir = tempfile::tempdir().unwrap();
      let task_dir = gest_dir.path().join(paths::TASK_DIR);
      fs::create_dir_all(&task_dir).unwrap();

      let fake_id: Id = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk".parse().unwrap();
      let task_path = paths::task_path(gest_dir.path(), &fake_id);
      fs::write(&task_path, "id: kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\ntitle: Active\n").unwrap();

      let dirs = vec![(pid.clone(), gest_dir.path().to_path_buf())];
      let result = orphan_tombstones(&conn, &Scope::Project(pid), &dirs).await.unwrap();

      assert_eq!(result.count, 0);
    }

    #[tokio::test]
    async fn it_finds_orphan_artifact_tombstones() {
      let (_store, conn, _tmp, pid) = setup().await;

      let gest_dir = tempfile::tempdir().unwrap();
      let artifact_dir = gest_dir.path().join(paths::ARTIFACT_DIR);
      fs::create_dir_all(&artifact_dir).unwrap();

      let fake_id: Id = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk".parse().unwrap();
      let artifact_path = paths::artifact_path(gest_dir.path(), &fake_id);
      fs::write(
        &artifact_path,
        "---\nid: kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\ntitle: Ghost Spec\ndeleted_at: 2026-04-01T00:00:00Z\n---\n# Body\n",
      )
      .unwrap();

      let dirs = vec![(pid.clone(), gest_dir.path().to_path_buf())];
      let result = orphan_tombstones(&conn, &Scope::Project(pid), &dirs).await.unwrap();

      assert_eq!(result.count, 1);
      assert_eq!(result.paths[0], artifact_path);
    }

    #[tokio::test]
    async fn it_scopes_to_project_when_multiple_dirs() {
      let (_store, conn, _tmp, pid1, pid2) = setup_two_projects().await;

      let gd1 = tempfile::tempdir().unwrap();
      let gd2 = tempfile::tempdir().unwrap();

      for gd in [&gd1, &gd2] {
        let task_dir = gd.path().join(paths::TASK_DIR);
        fs::create_dir_all(&task_dir).unwrap();
        let fake_id: Id = Id::new();
        let task_path = paths::task_path(gd.path(), &fake_id);
        fs::write(
          &task_path,
          format!("id: {fake_id}\ntitle: Ghost\ndeleted_at: 2026-04-01T00:00:00Z\n"),
        )
        .unwrap();
      }

      let dirs = vec![
        (pid1.clone(), gd1.path().to_path_buf()),
        (pid2.clone(), gd2.path().to_path_buf()),
      ];

      let result_p1 = orphan_tombstones(&conn, &Scope::Project(pid1), &dirs).await.unwrap();

      assert_eq!(result_p1.count, 1);

      let result_all = orphan_tombstones(&conn, &Scope::AllProjects, &dirs).await.unwrap();

      assert_eq!(result_all.count, 2);
    }

    #[tokio::test]
    async fn it_handles_missing_gest_dirs_gracefully() {
      let (_store, conn, _tmp, pid) = setup().await;

      let dirs = vec![(pid.clone(), PathBuf::from("/nonexistent/.gest"))];
      let result = orphan_tombstones(&conn, &Scope::Project(pid), &dirs).await.unwrap();

      assert_eq!(result.count, 0);
    }
  }

  mod read_only_invariant {
    use super::*;

    #[tokio::test]
    async fn it_does_not_modify_database_state() {
      let (_store, conn, _tmp, pid) = setup().await;

      // Insert some data
      let t = task::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "Task".into(),
          status: Some(crate::store::model::primitives::TaskStatus::Done),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let a = artifact::create(
        &conn,
        &pid,
        &crate::store::model::artifact::New {
          title: "Spec".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      conn
        .execute(
          "UPDATE artifacts SET archived_at = datetime('now') WHERE id = ?1",
          [a.id().to_string()],
        )
        .await
        .unwrap();

      // Snapshot row counts before
      let task_count_before = count_rows(&conn, "tasks").await;
      let artifact_count_before = count_rows(&conn, "artifacts").await;
      let project_count_before = count_rows(&conn, "projects").await;
      let rel_count_before = count_rows(&conn, "relationships").await;

      // Run all selectors
      let scope = Scope::Project(pid);
      terminal_tasks(&conn, &scope).await.unwrap();
      terminal_iterations(&conn, &scope).await.unwrap();
      archived_artifacts(&conn, &scope).await.unwrap();
      archived_projects(&conn, &scope).await.unwrap();
      dangling_relationships(&conn, &scope).await.unwrap();
      orphan_tombstones(&conn, &scope, &[]).await.unwrap();

      // Verify nothing changed
      assert_eq!(count_rows(&conn, "tasks").await, task_count_before);
      assert_eq!(count_rows(&conn, "artifacts").await, artifact_count_before);
      assert_eq!(count_rows(&conn, "projects").await, project_count_before);
      assert_eq!(count_rows(&conn, "relationships").await, rel_count_before);
      drop(t);
    }
  }

  mod terminal_iterations_fn {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{iteration::New, primitives::IterationStatus};

    #[tokio::test]
    async fn it_returns_terminal_iterations_for_project() {
      let (_store, conn, _tmp, pid) = setup().await;

      let active = iteration::create(
        &conn,
        &pid,
        &New {
          title: "Active".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let completed = iteration::create(
        &conn,
        &pid,
        &New {
          title: "Completed".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration::update(
        &conn,
        completed.id(),
        &crate::store::model::iteration::Patch {
          status: Some(IterationStatus::Completed),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let cancelled = iteration::create(
        &conn,
        &pid,
        &New {
          title: "Cancelled".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration::update(
        &conn,
        cancelled.id(),
        &crate::store::model::iteration::Patch {
          status: Some(IterationStatus::Cancelled),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let result = terminal_iterations(&conn, &Scope::Project(pid)).await.unwrap();

      assert_eq!(result.total(), 2);
      assert_eq!(result.completed, 1);
      assert_eq!(result.cancelled, 1);
      assert_eq!(result.ids.len(), 2);
      drop(active);
    }

    #[tokio::test]
    async fn it_returns_terminal_iterations_across_projects() {
      let (_store, conn, _tmp, pid1, pid2) = setup_two_projects().await;

      let i1 = iteration::create(
        &conn,
        &pid1,
        &New {
          title: "I1".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration::update(
        &conn,
        i1.id(),
        &crate::store::model::iteration::Patch {
          status: Some(IterationStatus::Completed),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let i2 = iteration::create(
        &conn,
        &pid2,
        &New {
          title: "I2".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration::update(
        &conn,
        i2.id(),
        &crate::store::model::iteration::Patch {
          status: Some(IterationStatus::Cancelled),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let result = terminal_iterations(&conn, &Scope::AllProjects).await.unwrap();

      assert_eq!(result.total(), 2);
    }

    #[tokio::test]
    async fn it_returns_empty_when_no_terminal_iterations() {
      let (_store, conn, _tmp, pid) = setup().await;

      iteration::create(
        &conn,
        &pid,
        &New {
          title: "Active".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let result = terminal_iterations(&conn, &Scope::Project(pid)).await.unwrap();

      assert_eq!(result.total(), 0);
      assert!(result.ids.is_empty());
    }
  }

  mod terminal_tasks_fn {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{primitives::TaskStatus, task::New};

    #[tokio::test]
    async fn it_returns_terminal_tasks_for_project() {
      let (_store, conn, _tmp, pid) = setup().await;

      task::create(
        &conn,
        &pid,
        &New {
          title: "Open".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      task::create(
        &conn,
        &pid,
        &New {
          title: "Done".into(),
          status: Some(TaskStatus::Done),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      task::create(
        &conn,
        &pid,
        &New {
          title: "Cancelled".into(),
          status: Some(TaskStatus::Cancelled),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let result = terminal_tasks(&conn, &Scope::Project(pid)).await.unwrap();

      assert_eq!(result.total(), 2);
      assert_eq!(result.done, 1);
      assert_eq!(result.cancelled, 1);
      assert_eq!(result.ids.len(), 2);
    }

    #[tokio::test]
    async fn it_returns_terminal_tasks_across_projects() {
      let (_store, conn, _tmp, pid1, pid2) = setup_two_projects().await;

      task::create(
        &conn,
        &pid1,
        &New {
          title: "Done P1".into(),
          status: Some(TaskStatus::Done),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      task::create(
        &conn,
        &pid2,
        &New {
          title: "Done P2".into(),
          status: Some(TaskStatus::Done),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let result = terminal_tasks(&conn, &Scope::AllProjects).await.unwrap();

      assert_eq!(result.total(), 2);
      assert_eq!(result.done, 2);
    }

    #[tokio::test]
    async fn it_returns_empty_when_no_terminal_tasks() {
      let (_store, conn, _tmp, pid) = setup().await;

      task::create(
        &conn,
        &pid,
        &New {
          title: "Open".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let result = terminal_tasks(&conn, &Scope::Project(pid)).await.unwrap();

      assert_eq!(result.total(), 0);
      assert!(result.ids.is_empty());
    }
  }

  async fn count_rows(conn: &Connection, table: &str) -> i64 {
    let mut rows = conn.query(&format!("SELECT COUNT(*) FROM {table}"), ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    row.get(0).unwrap()
  }
}
