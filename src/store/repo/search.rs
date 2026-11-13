//! Cross-entity search: turns a parsed query into SQL over tasks, artifacts and iterations.
//!
//! [`query`] runs at most one parameterized `SELECT` per entity type that the query's `is:` filters allow, and
//! returns the three result lists together.

use libsql::{Connection, Value};

use crate::store::{
  Error,
  model::{artifact, iteration, primitives::Id, task},
  search_query::{Filter, ParsedQuery},
};

/// Results from a cross-entity search.
pub struct Results {
  /// Artifacts matching the query.
  pub artifacts: Vec<artifact::Model>,
  /// Iterations matching the query.
  pub iterations: Vec<iteration::Model>,
  /// Tasks matching the query.
  pub tasks: Vec<task::Model>,
}

/// Search across tasks, artifacts, and iterations using parsed query filters.
///
/// Translates `ParsedQuery` filters into SQL `WHERE` clauses:
/// - `is:` filters scope which entity types are queried
/// - `tag:` filters use `EXISTS` subqueries on `entity_tags`
/// - `status:` filters use `IN` / `NOT IN` clauses
/// - Free text uses `LIKE` matching on title and description/body
/// - `show_all=false` excludes resolved tasks, archived artifacts, and completed or cancelled iterations
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project to search
/// * `parsed` - the parsed query with its include and exclude filters and free text
/// * `show_all` - whether to keep resolved, archived and finished entities
///
/// # Returns
///
/// The matching tasks, artifacts and iterations, each list newest first and empty when its type is filtered
/// out, or an error when a query or row decoding fails.
pub async fn query(conn: &Connection, project_id: &Id, parsed: &ParsedQuery, show_all: bool) -> Result<Results, Error> {
  log::debug!("repo::search::query");
  let want_tasks = wants_entity_type(parsed, "task");
  let want_artifacts = wants_entity_type(parsed, "artifact");
  let want_iterations = wants_entity_type(parsed, "iteration");

  let tasks = if want_tasks {
    query_tasks(conn, project_id, parsed, show_all).await?
  } else {
    Vec::new()
  };

  let artifacts = if want_artifacts {
    query_artifacts(conn, project_id, parsed, show_all).await?
  } else {
    Vec::new()
  };

  let iterations = if want_iterations {
    query_iterations(conn, project_id, parsed, show_all).await?
  } else {
    Vec::new()
  };

  Ok(Results {
    artifacts,
    iterations,
    tasks,
  })
}

/// Determine whether a given entity type should be included based on `is:` filters.
///
/// # Arguments
///
/// * `parsed` - the parsed query
/// * `entity` - the entity type name, such as `task`, `artifact` or `iteration`
///
/// # Returns
///
/// `false` when the query has `is:` includes that do not name `entity`, or an `is:` exclude that does;
/// `true` otherwise.
fn wants_entity_type(parsed: &ParsedQuery, entity: &str) -> bool {
  let has_is_include = parsed.include.iter().any(|f| matches!(f, Filter::Is(_)));

  if has_is_include && !parsed.include.iter().any(|f| matches!(f, Filter::Is(v) if v == entity)) {
    return false;
  }

  !parsed.exclude.iter().any(|f| matches!(f, Filter::Is(v) if v == entity))
}

/// Shared state for building a parameterized query.
struct QueryBuilder {
  /// `WHERE` conditions, joined with `AND`; the first restricts to the project.
  conditions: Vec<String>,
  /// Positional parameter values, in the order of their `?N` placeholders.
  params: Vec<Value>,
  /// The number of the next `?N` placeholder; starts at 2 because `?1` is the project id.
  idx: usize,
}

impl QueryBuilder {
  /// Start a builder whose only condition restricts rows to one project, bound as `?1`.
  ///
  /// # Arguments
  ///
  /// * `project_id` - the project to restrict to
  ///
  /// # Returns
  ///
  /// A builder with the project condition and parameter, and the next placeholder number at 2.
  fn new(project_id: &Id) -> Self {
    Self {
      conditions: vec!["project_id = ?1".to_string()],
      params: vec![Value::from(project_id.to_string())],
      idx: 2,
    }
  }

  /// Take the next positional placeholder number and advance the counter.
  ///
  /// # Returns
  ///
  /// The placeholder number to use as `?N`.
  fn next_param(&mut self) -> usize {
    let i = self.idx;
    self.idx += 1;
    i
  }

  /// Add tag include filters (OR-combined).
  ///
  /// # Arguments
  ///
  /// * `parsed` - the parsed query whose `tag:` includes are read
  /// * `table` - the SQL table of the searched entity, used to correlate the subquery
  /// * `entity_type` - the `entity_tags.entity_type` value of that entity
  fn add_tag_includes(&mut self, parsed: &ParsedQuery, table: &str, entity_type: &str) {
    let tags: Vec<&str> = parsed
      .include
      .iter()
      .filter_map(|f| match f {
        Filter::Tag(v) => Some(v.as_str()),
        _ => None,
      })
      .collect();

    if tags.is_empty() {
      return;
    }

    // OR-combine: EXISTS for tag1 OR EXISTS for tag2 ...
    let mut exists_clauses = Vec::new();
    for tag in tags {
      let i = self.next_param();
      exists_clauses.push(format!(
        "EXISTS (SELECT 1 FROM entity_tags \
          JOIN tags ON tags.id = entity_tags.tag_id \
          WHERE tags.label = ?{i} \
          AND entity_tags.entity_id = {table}.id \
          AND entity_tags.entity_type = '{entity_type}')"
      ));
      self.params.push(Value::from(tag.to_string()));
    }

    self.conditions.push(format!("({})", exists_clauses.join(" OR ")));
  }

  /// Add tag exclude filters (AND-combined).
  ///
  /// # Arguments
  ///
  /// * `parsed` - the parsed query whose `tag:` excludes are read
  /// * `table` - the SQL table of the searched entity, used to correlate the subquery
  /// * `entity_type` - the `entity_tags.entity_type` value of that entity
  fn add_tag_excludes(&mut self, parsed: &ParsedQuery, table: &str, entity_type: &str) {
    let tags: Vec<&str> = parsed
      .exclude
      .iter()
      .filter_map(|f| match f {
        Filter::Tag(v) => Some(v.as_str()),
        _ => None,
      })
      .collect();

    for tag in tags {
      let i = self.next_param();
      self.conditions.push(format!(
        "NOT EXISTS (SELECT 1 FROM entity_tags \
          JOIN tags ON tags.id = entity_tags.tag_id \
          WHERE tags.label = ?{i} \
          AND entity_tags.entity_id = {table}.id \
          AND entity_tags.entity_type = '{entity_type}')"
      ));
      self.params.push(Value::from(tag.to_string()));
    }
  }

  /// Add status include filters (OR-combined via `IN`).
  ///
  /// # Arguments
  ///
  /// * `parsed` - the parsed query whose `status:` includes are read
  fn add_status_includes(&mut self, parsed: &ParsedQuery) {
    let statuses: Vec<&str> = parsed
      .include
      .iter()
      .filter_map(|f| match f {
        Filter::Status(v) => Some(v.as_str()),
        _ => None,
      })
      .collect();

    if statuses.is_empty() {
      return;
    }

    let placeholders: Vec<String> = statuses
      .iter()
      .map(|s| {
        let i = self.next_param();
        self.params.push(Value::from(s.to_string()));
        format!("?{i}")
      })
      .collect();

    self
      .conditions
      .push(format!("LOWER(status) IN ({})", placeholders.join(", ")));
  }

  /// Add status exclude filters (AND-combined via `NOT IN`).
  ///
  /// # Arguments
  ///
  /// * `parsed` - the parsed query whose `status:` excludes are read
  fn add_status_excludes(&mut self, parsed: &ParsedQuery) {
    let statuses: Vec<&str> = parsed
      .exclude
      .iter()
      .filter_map(|f| match f {
        Filter::Status(v) => Some(v.as_str()),
        _ => None,
      })
      .collect();

    if statuses.is_empty() {
      return;
    }

    let placeholders: Vec<String> = statuses
      .iter()
      .map(|s| {
        let i = self.next_param();
        self.params.push(Value::from(s.to_string()));
        format!("?{i}")
      })
      .collect();

    self
      .conditions
      .push(format!("LOWER(status) NOT IN ({})", placeholders.join(", ")));
  }

  /// Add free text filter (title LIKE OR description/body LIKE).
  ///
  /// The query's text words are joined with spaces and matched as one substring.
  ///
  /// # Arguments
  ///
  /// * `parsed` - the parsed query whose free text is read
  /// * `text_columns` - the columns to match with `LIKE`, any of which may match
  fn add_text_filter(&mut self, parsed: &ParsedQuery, text_columns: &[&str]) {
    let text = parsed.text.join(" ");
    if text.is_empty() {
      return;
    }

    let i = self.next_param();
    let like_clauses: Vec<String> = text_columns.iter().map(|col| format!("{col} LIKE ?{i}")).collect();
    self.conditions.push(format!("({})", like_clauses.join(" OR ")));
    self.params.push(Value::from(format!("%{text}%")));
  }
}

/// Find the project's artifacts that match the query's tag and text filters.
///
/// Artifacts have no status, so any `status:` include yields no artifacts and `status:` excludes are ignored.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project to search
/// * `parsed` - the parsed query
/// * `show_all` - whether to keep archived artifacts
///
/// # Returns
///
/// The matching artifacts, newest first, or an error when the query or row decoding fails.
async fn query_artifacts(
  conn: &Connection,
  project_id: &Id,
  parsed: &ParsedQuery,
  show_all: bool,
) -> Result<Vec<artifact::Model>, Error> {
  // Artifacts have no status column — skip if status include filters are present.
  let has_status_include = parsed.include.iter().any(|f| matches!(f, Filter::Status(_)));
  if has_status_include {
    return Ok(Vec::new());
  }

  let mut qb = QueryBuilder::new(project_id);

  if !show_all {
    qb.conditions.push("archived_at IS NULL".to_string());
  }

  qb.add_tag_includes(parsed, "artifacts", "artifact");
  qb.add_tag_excludes(parsed, "artifacts", "artifact");
  // No status columns for artifacts — status excludes are no-ops.
  qb.add_text_filter(parsed, &["title", "body"]);

  let where_clause = qb.conditions.join(" AND ");
  let sql = format!(
    "SELECT id, project_id, title, body, metadata, \
      archived_at, created_at, updated_at \
      FROM artifacts WHERE {where_clause} ORDER BY created_at DESC"
  );

  let mut rows = conn.query(&sql, libsql::params_from_iter(qb.params)).await?;
  let mut items = Vec::new();
  while let Some(row) = rows.next().await? {
    items.push(artifact::Model::try_from(row)?);
  }
  Ok(items)
}

/// Find the project's iterations that match the query's tag, status and text filters.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project to search
/// * `parsed` - the parsed query
/// * `show_all` - whether to keep completed and cancelled iterations
///
/// # Returns
///
/// The matching iterations, newest first, or an error when the query or row decoding fails.
async fn query_iterations(
  conn: &Connection,
  project_id: &Id,
  parsed: &ParsedQuery,
  show_all: bool,
) -> Result<Vec<iteration::Model>, Error> {
  let mut qb = QueryBuilder::new(project_id);

  if !show_all {
    qb.conditions
      .push("status NOT IN ('completed', 'cancelled')".to_string());
  }

  qb.add_tag_includes(parsed, "iterations", "iteration");
  qb.add_tag_excludes(parsed, "iterations", "iteration");
  qb.add_status_includes(parsed);
  qb.add_status_excludes(parsed);
  qb.add_text_filter(parsed, &["title", "description"]);

  let where_clause = qb.conditions.join(" AND ");
  let sql = format!(
    "SELECT id, project_id, title, status, description, \
      metadata, completed_at, created_at, updated_at \
      FROM iterations WHERE {where_clause} ORDER BY created_at DESC"
  );

  let mut rows = conn.query(&sql, libsql::params_from_iter(qb.params)).await?;
  let mut items = Vec::new();
  while let Some(row) = rows.next().await? {
    items.push(iteration::Model::try_from(row)?);
  }
  Ok(items)
}

/// Find the project's tasks that match the query's tag, status and text filters.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project to search
/// * `parsed` - the parsed query
/// * `show_all` - whether to keep resolved tasks
///
/// # Returns
///
/// The matching tasks, newest first, or an error when the query or row decoding fails.
async fn query_tasks(
  conn: &Connection,
  project_id: &Id,
  parsed: &ParsedQuery,
  show_all: bool,
) -> Result<Vec<task::Model>, Error> {
  let mut qb = QueryBuilder::new(project_id);

  if !show_all {
    qb.conditions.push("resolved_at IS NULL".to_string());
  }

  qb.add_tag_includes(parsed, "tasks", "task");
  qb.add_tag_excludes(parsed, "tasks", "task");
  qb.add_status_includes(parsed);
  qb.add_status_excludes(parsed);
  qb.add_text_filter(parsed, &["title", "description"]);

  let where_clause = qb.conditions.join(" AND ");
  let sql = format!(
    "SELECT id, project_id, title, priority, status, description, \
      assigned_to, metadata, resolved_at, created_at, updated_at \
      FROM tasks WHERE {where_clause} ORDER BY created_at DESC"
  );

  let mut rows = conn.query(&sql, libsql::params_from_iter(qb.params)).await?;
  let mut items = Vec::new();
  while let Some(row) = rows.next().await? {
    items.push(task::Model::try_from(row)?);
  }
  Ok(items)
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use tempfile::TempDir;

  use super::*;
  use crate::store::{self, Db, model::Project, search_query};

  async fn setup() -> (Arc<Db>, Connection, TempDir, Id) {
    let (store, tmp) = store::open_temp().await.unwrap();
    let conn = store.connect().await.unwrap();
    let project = Project::new("/tmp/search-test".into());
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
    let pid = project.id().clone();
    (store, conn, tmp, pid)
  }

  /// Insert a task and optionally tag it.
  async fn insert_task(conn: &Connection, pid: &Id, title: &str, status: &str) -> Id {
    let id = Id::new();
    conn
      .execute(
        "INSERT INTO tasks (id, project_id, title, status) VALUES (?1, ?2, ?3, ?4)",
        [id.to_string(), pid.to_string(), title.to_string(), status.to_string()],
      )
      .await
      .unwrap();
    id
  }

  /// Insert an artifact.
  async fn insert_artifact(conn: &Connection, pid: &Id, title: &str, body: &str) -> Id {
    let id = Id::new();
    conn
      .execute(
        "INSERT INTO artifacts (id, project_id, title, body) VALUES (?1, ?2, ?3, ?4)",
        [id.to_string(), pid.to_string(), title.to_string(), body.to_string()],
      )
      .await
      .unwrap();
    id
  }

  /// Insert an iteration.
  async fn insert_iteration(conn: &Connection, pid: &Id, title: &str, status: &str) -> Id {
    let id = Id::new();
    conn
      .execute(
        "INSERT INTO iterations (id, project_id, title, status) VALUES (?1, ?2, ?3, ?4)",
        [id.to_string(), pid.to_string(), title.to_string(), status.to_string()],
      )
      .await
      .unwrap();
    id
  }

  /// Tag an entity.
  async fn tag_entity(conn: &Connection, entity_id: &Id, entity_type: &str, label: &str) {
    let tag_id = Id::new();
    conn
      .execute(
        "INSERT OR IGNORE INTO tags (id, label) VALUES (?1, ?2)",
        [tag_id.to_string(), label.to_string()],
      )
      .await
      .unwrap();
    // Fetch the actual tag id (may already exist).
    let mut rows = conn
      .query("SELECT id FROM tags WHERE label = ?1", [label.to_string()])
      .await
      .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let real_tag_id: String = row.get(0).unwrap();
    conn
      .execute(
        "INSERT INTO entity_tags (entity_id, entity_type, tag_id) VALUES (?1, ?2, ?3)",
        [entity_id.to_string(), entity_type.to_string(), real_tag_id],
      )
      .await
      .unwrap();
  }

  mod query_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_and_combines_across_filter_types() {
      let (_store, conn, _tmp, pid) = setup().await;
      let t1 = insert_task(&conn, &pid, "Tagged Open", "open").await;
      tag_entity(&conn, &t1, "task", "auth").await;
      let t2 = insert_task(&conn, &pid, "Tagged Done", "done").await;
      tag_entity(&conn, &t2, "task", "auth").await;

      let parsed = search_query::parse("tag:auth status:open");
      let results = query(&conn, &pid, &parsed, true).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.tasks[0].title(), "Tagged Open");
    }

    #[tokio::test]
    async fn it_excludes_archived_artifacts_by_default() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_artifact(&conn, &pid, "Active", "body").await;
      let archived_id = Id::new();
      conn
        .execute(
          "INSERT INTO artifacts (id, project_id, title, body, archived_at) \
            VALUES (?1, ?2, ?3, ?4, ?5)",
          [
            archived_id.to_string(),
            pid.to_string(),
            "Archived".to_string(),
            "body".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
          ],
        )
        .await
        .unwrap();

      let parsed = search_query::parse("");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.artifacts.len(), 1);
      assert_eq!(results.artifacts[0].title(), "Active");
    }

    #[tokio::test]
    async fn it_excludes_artifacts_when_status_filter_present() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "Open task", "open").await;
      insert_artifact(&conn, &pid, "Some artifact", "body").await;

      let parsed = search_query::parse("status:open");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.artifacts.len(), 0);
    }

    #[tokio::test]
    async fn it_excludes_done_with_negated_status() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "Open task", "open").await;
      insert_task(&conn, &pid, "Done task", "done").await;

      let parsed = search_query::parse("-status:done");
      let results = query(&conn, &pid, &parsed, true).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.tasks[0].title(), "Open task");
    }

    #[tokio::test]
    async fn it_excludes_entity_type_with_negated_is() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "My task", "open").await;
      insert_artifact(&conn, &pid, "My artifact", "body").await;

      let parsed = search_query::parse("-is:artifact");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.artifacts.len(), 0);
    }

    #[tokio::test]
    async fn it_excludes_resolved_tasks_by_default() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "Open task", "open").await;
      // Insert a resolved task (has resolved_at set).
      let done_id = Id::new();
      conn
        .execute(
          "INSERT INTO tasks (id, project_id, title, status, resolved_at) \
            VALUES (?1, ?2, ?3, ?4, ?5)",
          [
            done_id.to_string(),
            pid.to_string(),
            "Done task".to_string(),
            "done".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
          ],
        )
        .await
        .unwrap();

      let parsed = search_query::parse("");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.tasks[0].title(), "Open task");
    }

    #[tokio::test]
    async fn it_finds_tasks_by_title() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "Fix login bug", "open").await;
      insert_task(&conn, &pid, "Add feature", "open").await;

      let parsed = search_query::parse("login");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.artifacts.len(), 0);
      assert_eq!(results.iterations.len(), 0);
    }

    #[tokio::test]
    async fn it_includes_archived_artifacts_with_show_all() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_artifact(&conn, &pid, "Active", "body").await;
      let archived_id = Id::new();
      conn
        .execute(
          "INSERT INTO artifacts (id, project_id, title, body, archived_at) \
            VALUES (?1, ?2, ?3, ?4, ?5)",
          [
            archived_id.to_string(),
            pid.to_string(),
            "Archived".to_string(),
            "body".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
          ],
        )
        .await
        .unwrap();

      let parsed = search_query::parse("");
      let results = query(&conn, &pid, &parsed, true).await.unwrap();

      assert_eq!(results.artifacts.len(), 2);
    }

    #[tokio::test]
    async fn it_includes_resolved_tasks_with_show_all() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "Open task", "open").await;
      let done_id = Id::new();
      conn
        .execute(
          "INSERT INTO tasks (id, project_id, title, status, resolved_at) \
            VALUES (?1, ?2, ?3, ?4, ?5)",
          [
            done_id.to_string(),
            pid.to_string(),
            "Done task".to_string(),
            "done".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
          ],
        )
        .await
        .unwrap();

      let parsed = search_query::parse("");
      let results = query(&conn, &pid, &parsed, true).await.unwrap();

      assert_eq!(results.tasks.len(), 2);
    }

    #[tokio::test]
    async fn it_or_combines_multiple_tag_filters() {
      let (_store, conn, _tmp, pid) = setup().await;
      let t1 = insert_task(&conn, &pid, "Task A", "open").await;
      tag_entity(&conn, &t1, "task", "auth").await;
      let t2 = insert_task(&conn, &pid, "Task B", "open").await;
      tag_entity(&conn, &t2, "task", "login").await;
      insert_task(&conn, &pid, "Task C", "open").await;

      let parsed = search_query::parse("tag:auth tag:login");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 2);
    }

    #[tokio::test]
    async fn it_returns_only_entities_tagged_auth() {
      let (_store, conn, _tmp, pid) = setup().await;
      let t1 = insert_task(&conn, &pid, "Auth task", "open").await;
      tag_entity(&conn, &t1, "task", "auth").await;
      insert_task(&conn, &pid, "Other task", "open").await;

      let parsed = search_query::parse("tag:auth");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.tasks[0].title(), "Auth task");
    }

    #[tokio::test]
    async fn it_returns_only_open_tasks_with_is_and_status() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "Open task", "open").await;
      insert_task(&conn, &pid, "In progress task", "in_progress").await;
      insert_artifact(&conn, &pid, "Some artifact", "body").await;

      let parsed = search_query::parse("is:task status:open");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.tasks[0].title(), "Open task");
      assert_eq!(results.artifacts.len(), 0);
      assert_eq!(results.iterations.len(), 0);
    }

    #[tokio::test]
    async fn it_scopes_to_is_task_only() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "My task", "open").await;
      insert_artifact(&conn, &pid, "My artifact", "body").await;
      insert_iteration(&conn, &pid, "My iteration", "active").await;

      let parsed = search_query::parse("is:task");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.artifacts.len(), 0);
      assert_eq!(results.iterations.len(), 0);
    }

    #[tokio::test]
    async fn it_searches_across_entity_types() {
      let (_store, conn, _tmp, pid) = setup().await;
      insert_task(&conn, &pid, "auth task", "open").await;
      insert_artifact(&conn, &pid, "auth spec", "body").await;

      let parsed = search_query::parse("auth");
      let results = query(&conn, &pid, &parsed, false).await.unwrap();

      assert_eq!(results.tasks.len(), 1);
      assert_eq!(results.artifacts.len(), 1);
    }
  }
}
