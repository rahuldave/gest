//! Cascade delete for top-level entities (artifacts, iterations, tasks).
//!
//! This helper performs a hard delete of an entity together with every
//! dependent row (notes, entity_tags, relationships, iteration_tasks),
//! recording a `deleted` event for each row via the transaction log so the
//! operation can be undone.
//!
//! Event ordering is intentional: children are recorded *before* the parent
//! so that the reverse-order undo replay re-inserts the parent row first and
//! every child row afterwards (whose FKs reference the parent). The forward
//! delete uses the same child-first ordering so FK constraints are never
//! violated during the mutation itself.
//!
//! This module does **not** write tombstone files, prompt the user, or
//! begin a transaction — those concerns belong to the caller.
//!
//! Captured children are modeled as a typed [`CapturedChild`] enum rather
//! than a loose `serde_json::Map`. This keeps the capture → delete → audit
//! pipeline compile-time checked: adding a new column to a child table
//! forces a corresponding change in the enum variant, and the audit JSON
//! payload is only constructed once, at the point where it is handed to
//! [`transaction::record_event`].

use libsql::{Connection, Value};
use serde_json::{Map, Value as JsonValue};

use crate::store::{
  Error,
  model::primitives::{EntityType, Id},
  repo::{artifact, iteration, task, transaction},
};

/// Per-table counts of rows removed by a [`delete_with_cascade`] call.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeleteReport {
  /// Number of `iteration_tasks` join rows removed.
  pub iteration_tasks: usize,
  /// Number of attached notes removed.
  pub notes: usize,
  /// Number of relationships (in either direction) removed.
  pub relationships: usize,
  /// Number of `entity_tags` attachments removed.
  pub tags: usize,
}

/// A typed snapshot of a dependent row captured prior to deletion.
///
/// Each variant carries the exact columns required to both (a) re-emit the
/// row as an audit payload whose shape matches the on-disk format expected
/// by transaction undo replay, and (b) issue a typed `DELETE` statement
/// without having to round-trip through `serde_json`.
enum CapturedChild {
  /// An `entity_tags` row that attaches a tag to the deleted entity.
  EntityTag {
    created_at: String,
    entity_id: String,
    entity_type: String,
    tag_id: String,
  },
  /// An `iteration_tasks` row that links the deleted task or iteration to its partner.
  IterationTask {
    created_at: String,
    iteration_id: String,
    phase: i64,
    task_id: String,
  },
  /// A `notes` row attached to the deleted entity.
  Note {
    author_id: Option<String>,
    body: String,
    created_at: String,
    entity_id: String,
    entity_type: String,
    id: String,
    updated_at: String,
  },
  /// A `relationships` row with the deleted entity as its source or its target.
  Relationship {
    created_at: String,
    id: String,
    rel_type: String,
    source_id: String,
    source_type: String,
    target_id: String,
    target_type: String,
    updated_at: String,
  },
}

impl CapturedChild {
  /// Logical primary key used to identify this row in the audit log.
  ///
  /// # Returns
  ///
  /// `entity_type:entity_id:tag_id` for an entity tag, `iteration_id:task_id` for an iteration task, and the row's
  /// own `id` for a note or a relationship.
  fn audit_row_id(&self) -> String {
    match self {
      Self::EntityTag {
        entity_id,
        entity_type,
        tag_id,
        ..
      } => format!("{entity_type}:{entity_id}:{tag_id}"),
      Self::IterationTask {
        iteration_id,
        task_id,
        ..
      } => format!("{iteration_id}:{task_id}"),
      Self::Note {
        id, ..
      }
      | Self::Relationship {
        id, ..
      } => id.clone(),
    }
  }

  /// Delete this captured row from its backing table.
  ///
  /// # Arguments
  ///
  /// * `conn` - the connection that runs the `DELETE`
  ///
  /// # Returns
  ///
  /// `Ok(())` once the statement has run, or the database error.
  async fn delete(&self, conn: &Connection) -> Result<(), Error> {
    match self {
      Self::EntityTag {
        entity_id,
        entity_type,
        tag_id,
        ..
      } => {
        conn
          .execute(
            "DELETE FROM entity_tags WHERE entity_type = ?1 AND entity_id = ?2 AND tag_id = ?3",
            libsql::params![entity_type.clone(), entity_id.clone(), tag_id.clone()],
          )
          .await?;
      }
      Self::IterationTask {
        iteration_id,
        task_id,
        ..
      } => {
        conn
          .execute(
            "DELETE FROM iteration_tasks WHERE iteration_id = ?1 AND task_id = ?2",
            libsql::params![iteration_id.clone(), task_id.clone()],
          )
          .await?;
      }
      Self::Note {
        id, ..
      } => {
        conn.execute("DELETE FROM notes WHERE id = ?1", [id.clone()]).await?;
      }
      Self::Relationship {
        id, ..
      } => {
        conn
          .execute("DELETE FROM relationships WHERE id = ?1", [id.clone()])
          .await?;
      }
    }
    Ok(())
  }

  /// Name of the backing SQL table this row was captured from.
  ///
  /// # Returns
  ///
  /// One of `entity_tags`, `iteration_tasks`, `notes` or `relationships`.
  fn table(&self) -> &'static str {
    match self {
      Self::EntityTag {
        ..
      } => "entity_tags",
      Self::IterationTask {
        ..
      } => "iteration_tasks",
      Self::Note {
        ..
      } => "notes",
      Self::Relationship {
        ..
      } => "relationships",
    }
  }

  /// Build the JSON payload stored in the audit log for this row.
  ///
  /// The shape here is load-bearing: [`transaction::undo`] reconstructs the
  /// row via an `INSERT` whose column list is the object's key set, so the
  /// set of keys and their value types must continue to match the backing
  /// table's schema.
  ///
  /// # Returns
  ///
  /// A JSON object with one key per column of the row; a note without an author has `author_id` set to `null`.
  fn to_audit_payload(&self) -> JsonValue {
    let mut map = Map::new();
    match self {
      Self::EntityTag {
        created_at,
        entity_id,
        entity_type,
        tag_id,
      } => {
        map.insert("entity_id".into(), JsonValue::String(entity_id.clone()));
        map.insert("entity_type".into(), JsonValue::String(entity_type.clone()));
        map.insert("tag_id".into(), JsonValue::String(tag_id.clone()));
        map.insert("created_at".into(), JsonValue::String(created_at.clone()));
      }
      Self::IterationTask {
        created_at,
        iteration_id,
        phase,
        task_id,
      } => {
        map.insert("iteration_id".into(), JsonValue::String(iteration_id.clone()));
        map.insert("task_id".into(), JsonValue::String(task_id.clone()));
        map.insert("phase".into(), JsonValue::Number((*phase).into()));
        map.insert("created_at".into(), JsonValue::String(created_at.clone()));
      }
      Self::Note {
        author_id,
        body,
        created_at,
        entity_id,
        entity_type,
        id,
        updated_at,
      } => {
        map.insert("id".into(), JsonValue::String(id.clone()));
        map.insert("entity_id".into(), JsonValue::String(entity_id.clone()));
        map.insert("entity_type".into(), JsonValue::String(entity_type.clone()));
        map.insert(
          "author_id".into(),
          match author_id {
            Some(a) => JsonValue::String(a.clone()),
            None => JsonValue::Null,
          },
        );
        map.insert("body".into(), JsonValue::String(body.clone()));
        map.insert("created_at".into(), JsonValue::String(created_at.clone()));
        map.insert("updated_at".into(), JsonValue::String(updated_at.clone()));
      }
      Self::Relationship {
        created_at,
        id,
        rel_type,
        source_id,
        source_type,
        target_id,
        target_type,
        updated_at,
      } => {
        map.insert("id".into(), JsonValue::String(id.clone()));
        map.insert("rel_type".into(), JsonValue::String(rel_type.clone()));
        map.insert("source_id".into(), JsonValue::String(source_id.clone()));
        map.insert("source_type".into(), JsonValue::String(source_type.clone()));
        map.insert("target_id".into(), JsonValue::String(target_id.clone()));
        map.insert("target_type".into(), JsonValue::String(target_type.clone()));
        map.insert("created_at".into(), JsonValue::String(created_at.clone()));
        map.insert("updated_at".into(), JsonValue::String(updated_at.clone()));
      }
    }
    JsonValue::Object(map)
  }
}

/// Delete an entity and all rows that depend on it, recording transaction
/// events for each removed row so the operation can be undone.
///
/// * Callers are expected to have already begun a transaction and resolved
///   `transaction_id`; this helper only appends events and issues `DELETE`
///   statements.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `transaction_id` - the transaction that the `deleted` events are recorded under
/// * `entity_type` - whether the entity is an artifact, an iteration or a task
/// * `entity_id` - `entity_id` must be the fully-resolved id of the entity.
///
/// # Returns
///
/// Returns a [`DeleteReport`] with the number of rows removed from each
/// dependent table. It is [`Error::NotFound`] when no entity has that id, or a database error.
pub async fn delete_with_cascade(
  conn: &Connection,
  transaction_id: &Id,
  entity_type: EntityType,
  entity_id: &Id,
) -> Result<DeleteReport, Error> {
  log::debug!("repo::entity::delete::delete_with_cascade");
  let entity_type_str = entity_type.to_string();
  let id_str = entity_id.to_string();

  // Collect every dependent row first. We intentionally capture children in
  // the order they must be restored by undo: notes, entity_tags,
  // relationships, iteration_tasks — i.e. leaves before the joining table
  // that references the parent.
  let notes = capture_notes(conn, &entity_type_str, &id_str).await?;
  let entity_tags = capture_entity_tags(conn, &entity_type_str, &id_str).await?;
  let relationships = capture_relationships(conn, &entity_type_str, &id_str).await?;
  let iteration_tasks = match entity_type {
    EntityType::Task => capture_iteration_tasks_for_task(conn, &id_str).await?,
    EntityType::Iteration => capture_iteration_tasks_for_iteration(conn, &id_str).await?,
    EntityType::Artifact => Vec::new(),
  };
  let parent = capture_parent(conn, entity_type, &id_str).await?;

  let report = DeleteReport {
    iteration_tasks: iteration_tasks.len(),
    notes: notes.len(),
    relationships: relationships.len(),
    tags: entity_tags.len(),
  };

  // Record + delete children first, parent last. Within each child group the
  // record-then-delete pair keeps the audit log consistent with the physical
  // mutation.
  for child in notes
    .iter()
    .chain(entity_tags.iter())
    .chain(relationships.iter())
    .chain(iteration_tasks.iter())
  {
    record_and_delete_child(conn, transaction_id, child).await?;
  }

  record_and_delete_parent(conn, transaction_id, entity_type, &parent, &id_str).await?;

  Ok(report)
}

/// Batched equivalent of [`delete_with_cascade`] for a slice of same-type ids.
///
/// Captures every dependent row (notes, entity_tags, relationships,
/// iteration_tasks) for the full id set in a single `IN (...)` query per child
/// table, records one audit event per captured row, then issues a single
/// batched `DELETE ... WHERE ... IN (...)` per table. The overall ordering is
/// identical to [`delete_with_cascade`]: children first, parents last, so FK
/// constraints stay satisfied and undo replay restores rows in reverse.
///
/// # Arguments
///
/// * `conn` - the connection, inside a transaction the caller has begun
/// * `transaction_id` - the transaction that the `deleted` events are recorded under
/// * `entity_type` - the type that every id in `entity_ids` has
/// * `entity_ids` - the fully-resolved ids of the entities to delete
///
/// # Returns
///
/// Returns a combined [`DeleteReport`] covering every deleted row across the
/// whole batch. An empty slice returns a zero report without issuing any
/// queries. It is [`Error::NotFound`] when any id has no entity, or a database error.
pub async fn delete_many_with_cascade(
  conn: &Connection,
  transaction_id: &Id,
  entity_type: EntityType,
  entity_ids: &[&Id],
) -> Result<DeleteReport, Error> {
  log::debug!("repo::entity::delete::delete_many_with_cascade");
  if entity_ids.is_empty() {
    return Ok(DeleteReport::default());
  }

  let entity_type_str = entity_type.to_string();
  let id_strs: Vec<String> = entity_ids.iter().map(|id| id.to_string()).collect();

  let notes = capture_notes_many(conn, &entity_type_str, &id_strs).await?;
  let entity_tags = capture_entity_tags_many(conn, &entity_type_str, &id_strs).await?;
  let relationships = capture_relationships_many(conn, &entity_type_str, &id_strs).await?;
  let iteration_tasks = match entity_type {
    EntityType::Task => capture_iteration_tasks_for_tasks(conn, &id_strs).await?,
    EntityType::Iteration => capture_iteration_tasks_for_iterations(conn, &id_strs).await?,
    EntityType::Artifact => Vec::new(),
  };
  let parents = capture_parents_many(conn, entity_type, &id_strs).await?;

  let report = DeleteReport {
    iteration_tasks: iteration_tasks.len(),
    notes: notes.len(),
    relationships: relationships.len(),
    tags: entity_tags.len(),
  };

  // Record every child event first so the audit log order matches the
  // child-first mutation we're about to issue.
  for child in notes
    .iter()
    .chain(entity_tags.iter())
    .chain(relationships.iter())
    .chain(iteration_tasks.iter())
  {
    let payload = child.to_audit_payload();
    transaction::record_event(
      conn,
      transaction_id,
      child.table(),
      &child.audit_row_id(),
      "deleted",
      Some(&payload),
    )
    .await?;
  }

  // Delete all captured children in one query per table.
  delete_notes_many(conn, &entity_type_str, &id_strs, !notes.is_empty()).await?;
  delete_entity_tags_many(conn, &entity_type_str, &id_strs, !entity_tags.is_empty()).await?;
  delete_relationships_many(conn, &entity_type_str, &id_strs, !relationships.is_empty()).await?;
  if !iteration_tasks.is_empty() {
    match entity_type {
      EntityType::Task => delete_iteration_tasks_for_tasks(conn, &id_strs).await?,
      EntityType::Iteration => delete_iteration_tasks_for_iterations(conn, &id_strs).await?,
      EntityType::Artifact => {}
    }
  }

  // Record + delete parents.
  for (id_str, payload) in &parents {
    let (table, _) = parent_table_and_sql(entity_type);
    transaction::record_event(conn, transaction_id, table, id_str, "deleted", Some(payload)).await?;
  }
  delete_parents_many(conn, entity_type, &id_strs).await?;

  Ok(report)
}

/// Read the `entity_tags` rows of one entity, before they are deleted.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `entity_type` - the entity's type, as stored in `entity_tags.entity_type`
/// * `entity_id` - the entity's full id
///
/// # Returns
///
/// One [`CapturedChild::EntityTag`] per tag attached to the entity, possibly none, or a database error.
async fn capture_entity_tags(
  conn: &Connection,
  entity_type: &str,
  entity_id: &str,
) -> Result<Vec<CapturedChild>, Error> {
  let mut rows = conn
    .query(
      "SELECT entity_id, entity_type, tag_id, created_at \
        FROM entity_tags WHERE entity_type = ?1 AND entity_id = ?2",
      libsql::params![entity_type.to_string(), entity_id.to_string()],
    )
    .await?;

  let mut captured = Vec::new();
  while let Some(row) = rows.next().await? {
    captured.push(CapturedChild::EntityTag {
      entity_id: row.get(0)?,
      entity_type: row.get(1)?,
      tag_id: row.get(2)?,
      created_at: row.get(3)?,
    });
  }
  Ok(captured)
}

/// Read the `iteration_tasks` rows that put tasks in one iteration, before they are deleted.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `iteration_id` - the iteration's full id
///
/// # Returns
///
/// One [`CapturedChild::IterationTask`] per task in the iteration, possibly none, or a database error.
async fn capture_iteration_tasks_for_iteration(
  conn: &Connection,
  iteration_id: &str,
) -> Result<Vec<CapturedChild>, Error> {
  let mut rows = conn
    .query(
      "SELECT iteration_id, task_id, phase, created_at \
        FROM iteration_tasks WHERE iteration_id = ?1",
      [iteration_id.to_string()],
    )
    .await?;

  collect_iteration_task_rows(&mut rows).await
}

/// Read the `iteration_tasks` rows that put one task in iterations, before they are deleted.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `task_id` - the task's full id
///
/// # Returns
///
/// One [`CapturedChild::IterationTask`] per iteration that holds the task, possibly none, or a database error.
async fn capture_iteration_tasks_for_task(conn: &Connection, task_id: &str) -> Result<Vec<CapturedChild>, Error> {
  let mut rows = conn
    .query(
      "SELECT iteration_id, task_id, phase, created_at \
        FROM iteration_tasks WHERE task_id = ?1",
      [task_id.to_string()],
    )
    .await?;

  collect_iteration_task_rows(&mut rows).await
}

/// Read the notes attached to one entity, before they are deleted.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `entity_type` - the entity's type, as stored in `notes.entity_type`
/// * `entity_id` - the entity's full id
///
/// # Returns
///
/// One [`CapturedChild::Note`] per note on the entity, possibly none, or a database error.
async fn capture_notes(conn: &Connection, entity_type: &str, entity_id: &str) -> Result<Vec<CapturedChild>, Error> {
  let mut rows = conn
    .query(
      "SELECT id, entity_id, entity_type, author_id, body, created_at, updated_at \
        FROM notes WHERE entity_type = ?1 AND entity_id = ?2",
      libsql::params![entity_type.to_string(), entity_id.to_string()],
    )
    .await?;

  let mut captured = Vec::new();
  while let Some(row) = rows.next().await? {
    captured.push(CapturedChild::Note {
      id: row.get(0)?,
      entity_id: row.get(1)?,
      entity_type: row.get(2)?,
      author_id: row.get(3)?,
      body: row.get(4)?,
      created_at: row.get(5)?,
      updated_at: row.get(6)?,
    });
  }
  Ok(captured)
}

/// Read the entity's own row as a JSON object, for its `deleted` audit event.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `entity_type` - picks the table and its `SELECT_COLUMNS`
/// * `id` - the entity's full id
///
/// # Returns
///
/// An object keyed by the table's `SELECT_COLUMNS`, [`Error::NotFound`] when no row has the id, or a database
/// error.
async fn capture_parent(conn: &Connection, entity_type: EntityType, id: &str) -> Result<JsonValue, Error> {
  let (table, select_columns) = match entity_type {
    EntityType::Artifact => ("artifacts", artifact::SELECT_COLUMNS),
    EntityType::Iteration => ("iterations", iteration::SELECT_COLUMNS),
    EntityType::Task => ("tasks", task::SELECT_COLUMNS),
  };
  let columns: Vec<&str> = select_columns.split(',').map(str::trim).collect();
  let sql = format!("SELECT {select_columns} FROM {table} WHERE id = ?1");

  let mut rows = conn.query(&sql, [id.to_string()]).await?;
  let row = rows
    .next()
    .await?
    .ok_or_else(|| Error::NotFound(format!("{entity_type} {id}")))?;

  let mut map = Map::new();
  for (idx, column) in columns.iter().enumerate() {
    let value: Value = row.get(idx as i32)?;
    map.insert((*column).to_string(), libsql_value_to_json(value));
  }
  Ok(JsonValue::Object(map))
}

/// Read the relationships with the entity as source or target, before they are deleted.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `entity_type` - the entity's type, matched against `source_type` or `target_type`
/// * `entity_id` - the entity's full id
///
/// # Returns
///
/// One [`CapturedChild::Relationship`] per relationship in either direction, possibly none, or a database error.
async fn capture_relationships(
  conn: &Connection,
  entity_type: &str,
  entity_id: &str,
) -> Result<Vec<CapturedChild>, Error> {
  let mut rows = conn
    .query(
      "SELECT id, rel_type, source_id, source_type, target_id, target_type, created_at, updated_at \
        FROM relationships \
        WHERE (source_type = ?1 AND source_id = ?2) OR (target_type = ?1 AND target_id = ?2)",
      libsql::params![entity_type.to_string(), entity_id.to_string()],
    )
    .await?;

  let mut captured = Vec::new();
  while let Some(row) = rows.next().await? {
    captured.push(CapturedChild::Relationship {
      id: row.get(0)?,
      rel_type: row.get(1)?,
      source_id: row.get(2)?,
      source_type: row.get(3)?,
      target_id: row.get(4)?,
      target_type: row.get(5)?,
      created_at: row.get(6)?,
      updated_at: row.get(7)?,
    });
  }
  Ok(captured)
}

/// Turn rows of `iteration_id, task_id, phase, created_at` into captured children.
///
/// # Arguments
///
/// * `rows` - the query result, read to its end
///
/// # Returns
///
/// One [`CapturedChild::IterationTask`] per row, or the error from reading a row.
async fn collect_iteration_task_rows(rows: &mut libsql::Rows) -> Result<Vec<CapturedChild>, Error> {
  let mut captured = Vec::new();
  while let Some(row) = rows.next().await? {
    captured.push(CapturedChild::IterationTask {
      iteration_id: row.get(0)?,
      task_id: row.get(1)?,
      phase: row.get(2)?,
      created_at: row.get(3)?,
    });
  }
  Ok(captured)
}

/// Convert one column value to JSON for an audit payload.
///
/// A real that JSON cannot hold, such as NaN, becomes `null`. A blob becomes the text `<blob N bytes>`, so its bytes
/// are not kept.
///
/// # Arguments
///
/// * `value` - the column value from libsql
///
/// # Returns
///
/// The JSON value: null, a number or a string.
fn libsql_value_to_json(value: Value) -> JsonValue {
  match value {
    Value::Null => JsonValue::Null,
    Value::Integer(i) => JsonValue::Number(i.into()),
    Value::Real(f) => serde_json::Number::from_f64(f).map_or(JsonValue::Null, JsonValue::Number),
    Value::Text(s) => JsonValue::String(s),
    Value::Blob(b) => JsonValue::String(format!("<blob {} bytes>", b.len())),
  }
}

/// Record a `deleted` event for one captured child, then delete its row.
///
/// # Arguments
///
/// * `conn` - the connection, inside the caller's transaction
/// * `transaction_id` - the transaction that the event is recorded under
/// * `child` - the row to record and delete
///
/// # Returns
///
/// `Ok(())` once both have run, or the database error.
async fn record_and_delete_child(conn: &Connection, transaction_id: &Id, child: &CapturedChild) -> Result<(), Error> {
  let payload = child.to_audit_payload();
  transaction::record_event(
    conn,
    transaction_id,
    child.table(),
    &child.audit_row_id(),
    "deleted",
    Some(&payload),
  )
  .await?;
  child.delete(conn).await?;
  Ok(())
}

/// Record a `deleted` event for the entity's own row, then delete it.
///
/// # Arguments
///
/// * `conn` - the connection, inside the caller's transaction
/// * `transaction_id` - the transaction that the event is recorded under
/// * `entity_type` - picks the table
/// * `before` - the row as captured by [`capture_parent`], stored as the event's payload
/// * `id` - the entity's full id
///
/// # Returns
///
/// `Ok(())` once both have run, or the database error.
async fn record_and_delete_parent(
  conn: &Connection,
  transaction_id: &Id,
  entity_type: EntityType,
  before: &JsonValue,
  id: &str,
) -> Result<(), Error> {
  let (table, sql) = parent_table_and_sql(entity_type);

  transaction::record_event(conn, transaction_id, table, id, "deleted", Some(before)).await?;
  conn.execute(sql, [id.to_string()]).await?;
  Ok(())
}

/// Returns the `(table_name, single-row DELETE sql)` for an entity-parent table.
///
/// # Arguments
///
/// * `entity_type` - the type of the entity
///
/// # Returns
///
/// The table is `artifacts`, `iterations` or `tasks`, and the SQL is `DELETE ... WHERE id = ?1` on it.
fn parent_table_and_sql(entity_type: EntityType) -> (&'static str, &'static str) {
  match entity_type {
    EntityType::Artifact => ("artifacts", "DELETE FROM artifacts WHERE id = ?1"),
    EntityType::Iteration => ("iterations", "DELETE FROM iterations WHERE id = ?1"),
    EntityType::Task => ("tasks", "DELETE FROM tasks WHERE id = ?1"),
  }
}

/// Build the `?N, ?N+1, ...` placeholder list starting at the given 1-based index.
///
/// # Arguments
///
/// * `start` - the number of the first placeholder
/// * `count` - how many placeholders to write
///
/// # Returns
///
/// The placeholders joined by `, `, such as `?2, ?3, ?4`; empty when `count` is 0.
fn placeholders(start: usize, count: usize) -> String {
  (start..start + count)
    .map(|i| format!("?{i}"))
    .collect::<Vec<_>>()
    .join(", ")
}

/// Batched [`capture_entity_tags`]: read the `entity_tags` rows of many entities in one query.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `entity_type` - the type of every entity
/// * `entity_ids` - the entities' full ids
///
/// # Returns
///
/// One [`CapturedChild::EntityTag`] per row found; empty, with no query, when `entity_ids` is empty. Or a database
/// error.
async fn capture_entity_tags_many(
  conn: &Connection,
  entity_type: &str,
  entity_ids: &[String],
) -> Result<Vec<CapturedChild>, Error> {
  if entity_ids.is_empty() {
    return Ok(Vec::new());
  }
  let holders = placeholders(2, entity_ids.len());
  let sql = format!(
    "SELECT entity_id, entity_type, tag_id, created_at \
      FROM entity_tags WHERE entity_type = ?1 AND entity_id IN ({holders})"
  );
  let mut params: Vec<Value> = Vec::with_capacity(entity_ids.len() + 1);
  params.push(Value::from(entity_type.to_string()));
  for id in entity_ids {
    params.push(Value::from(id.clone()));
  }

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  let mut captured = Vec::new();
  while let Some(row) = rows.next().await? {
    captured.push(CapturedChild::EntityTag {
      entity_id: row.get(0)?,
      entity_type: row.get(1)?,
      tag_id: row.get(2)?,
      created_at: row.get(3)?,
    });
  }
  Ok(captured)
}

/// Batched [`capture_iteration_tasks_for_iteration`]: read the `iteration_tasks` rows of many iterations.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `iteration_ids` - the iterations' full ids
///
/// # Returns
///
/// One [`CapturedChild::IterationTask`] per row found; empty, with no query, when `iteration_ids` is empty. Or a
/// database error.
async fn capture_iteration_tasks_for_iterations(
  conn: &Connection,
  iteration_ids: &[String],
) -> Result<Vec<CapturedChild>, Error> {
  if iteration_ids.is_empty() {
    return Ok(Vec::new());
  }
  let holders = placeholders(1, iteration_ids.len());
  let sql = format!(
    "SELECT iteration_id, task_id, phase, created_at \
      FROM iteration_tasks WHERE iteration_id IN ({holders})"
  );
  let params: Vec<Value> = iteration_ids.iter().map(|id| Value::from(id.clone())).collect();

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  collect_iteration_task_rows(&mut rows).await
}

/// Batched [`capture_iteration_tasks_for_task`]: read the `iteration_tasks` rows of many tasks.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `task_ids` - the tasks' full ids
///
/// # Returns
///
/// One [`CapturedChild::IterationTask`] per row found; empty, with no query, when `task_ids` is empty. Or a
/// database error.
async fn capture_iteration_tasks_for_tasks(
  conn: &Connection,
  task_ids: &[String],
) -> Result<Vec<CapturedChild>, Error> {
  if task_ids.is_empty() {
    return Ok(Vec::new());
  }
  let holders = placeholders(1, task_ids.len());
  let sql = format!(
    "SELECT iteration_id, task_id, phase, created_at \
      FROM iteration_tasks WHERE task_id IN ({holders})"
  );
  let params: Vec<Value> = task_ids.iter().map(|id| Value::from(id.clone())).collect();

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  collect_iteration_task_rows(&mut rows).await
}

/// Batched [`capture_notes`]: read the notes of many entities in one query.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `entity_type` - the type of every entity
/// * `entity_ids` - the entities' full ids
///
/// # Returns
///
/// One [`CapturedChild::Note`] per note found; empty, with no query, when `entity_ids` is empty. Or a database
/// error.
async fn capture_notes_many(
  conn: &Connection,
  entity_type: &str,
  entity_ids: &[String],
) -> Result<Vec<CapturedChild>, Error> {
  if entity_ids.is_empty() {
    return Ok(Vec::new());
  }
  let holders = placeholders(2, entity_ids.len());
  let sql = format!(
    "SELECT id, entity_id, entity_type, author_id, body, created_at, updated_at \
      FROM notes WHERE entity_type = ?1 AND entity_id IN ({holders})"
  );
  let mut params: Vec<Value> = Vec::with_capacity(entity_ids.len() + 1);
  params.push(Value::from(entity_type.to_string()));
  for id in entity_ids {
    params.push(Value::from(id.clone()));
  }

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  let mut captured = Vec::new();
  while let Some(row) = rows.next().await? {
    captured.push(CapturedChild::Note {
      id: row.get(0)?,
      entity_id: row.get(1)?,
      entity_type: row.get(2)?,
      author_id: row.get(3)?,
      body: row.get(4)?,
      created_at: row.get(5)?,
      updated_at: row.get(6)?,
    });
  }
  Ok(captured)
}

/// Batched [`capture_parent`]: read many entities' own rows as JSON objects in one query.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `entity_type` - picks the table and its `SELECT_COLUMNS`
/// * `entity_ids` - the entities' full ids
///
/// # Returns
///
/// One `(id, row object)` pair per entity, in the order the query returns them; empty when `entity_ids` is empty.
/// It is [`Error::NotFound`] when any id has no row, or a database error.
async fn capture_parents_many(
  conn: &Connection,
  entity_type: EntityType,
  entity_ids: &[String],
) -> Result<Vec<(String, JsonValue)>, Error> {
  if entity_ids.is_empty() {
    return Ok(Vec::new());
  }
  let (table, select_columns) = match entity_type {
    EntityType::Artifact => ("artifacts", artifact::SELECT_COLUMNS),
    EntityType::Iteration => ("iterations", iteration::SELECT_COLUMNS),
    EntityType::Task => ("tasks", task::SELECT_COLUMNS),
  };
  let columns: Vec<&str> = select_columns.split(',').map(str::trim).collect();
  let id_column_idx = columns
    .iter()
    .position(|c| *c == "id")
    .expect("entity parent SELECT_COLUMNS must include `id`");

  let holders = placeholders(1, entity_ids.len());
  let sql = format!("SELECT {select_columns} FROM {table} WHERE id IN ({holders})");
  let params: Vec<Value> = entity_ids.iter().map(|id| Value::from(id.clone())).collect();

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  let mut captured = Vec::new();
  while let Some(row) = rows.next().await? {
    let mut map = Map::new();
    let mut id_value: Option<String> = None;
    for (idx, column) in columns.iter().enumerate() {
      let value: Value = row.get(idx as i32)?;
      if idx == id_column_idx
        && let Value::Text(ref s) = value
      {
        id_value = Some(s.clone());
      }
      map.insert((*column).to_string(), libsql_value_to_json(value));
    }
    let id = id_value.ok_or_else(|| Error::NotFound(format!("{entity_type} row missing id column")))?;
    captured.push((id, JsonValue::Object(map)));
  }

  if captured.len() != entity_ids.len() {
    let found: std::collections::HashSet<&str> = captured.iter().map(|(id, _)| id.as_str()).collect();
    for id in entity_ids {
      if !found.contains(id.as_str()) {
        return Err(Error::NotFound(format!("{entity_type} {id}")));
      }
    }
  }

  Ok(captured)
}

/// Batched [`capture_relationships`]: read the relationships of many entities in one query.
///
/// # Arguments
///
/// * `conn` - the connection to query
/// * `entity_type` - the type of every entity
/// * `entity_ids` - the entities' full ids, matched as source or target
///
/// # Returns
///
/// One [`CapturedChild::Relationship`] per relationship found; empty, with no query, when `entity_ids` is empty. Or
/// a database error.
async fn capture_relationships_many(
  conn: &Connection,
  entity_type: &str,
  entity_ids: &[String],
) -> Result<Vec<CapturedChild>, Error> {
  if entity_ids.is_empty() {
    return Ok(Vec::new());
  }
  let source_holders = placeholders(2, entity_ids.len());
  let target_holders = placeholders(2 + entity_ids.len(), entity_ids.len());
  let sql = format!(
    "SELECT id, rel_type, source_id, source_type, target_id, target_type, created_at, updated_at \
      FROM relationships \
      WHERE (source_type = ?1 AND source_id IN ({source_holders})) \
      OR (target_type = ?1 AND target_id IN ({target_holders}))"
  );
  let mut params: Vec<Value> = Vec::with_capacity(entity_ids.len() * 2 + 1);
  params.push(Value::from(entity_type.to_string()));
  for id in entity_ids {
    params.push(Value::from(id.clone()));
  }
  for id in entity_ids {
    params.push(Value::from(id.clone()));
  }

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  let mut captured = Vec::new();
  while let Some(row) = rows.next().await? {
    captured.push(CapturedChild::Relationship {
      id: row.get(0)?,
      rel_type: row.get(1)?,
      source_id: row.get(2)?,
      source_type: row.get(3)?,
      target_id: row.get(4)?,
      target_type: row.get(5)?,
      created_at: row.get(6)?,
      updated_at: row.get(7)?,
    });
  }
  Ok(captured)
}

/// Delete the `entity_tags` rows of many entities in one statement.
///
/// # Arguments
///
/// * `conn` - the connection, inside the caller's transaction
/// * `entity_type` - the type of every entity
/// * `entity_ids` - the entities' full ids
/// * `any` - whether any rows were captured; when false nothing runs
///
/// # Returns
///
/// `Ok(())` once the statement has run or been skipped, or the database error.
async fn delete_entity_tags_many(
  conn: &Connection,
  entity_type: &str,
  entity_ids: &[String],
  any: bool,
) -> Result<(), Error> {
  if !any {
    return Ok(());
  }
  let holders = placeholders(2, entity_ids.len());
  let sql = format!("DELETE FROM entity_tags WHERE entity_type = ?1 AND entity_id IN ({holders})");
  let mut params: Vec<Value> = Vec::with_capacity(entity_ids.len() + 1);
  params.push(Value::from(entity_type.to_string()));
  for id in entity_ids {
    params.push(Value::from(id.clone()));
  }
  conn.execute(&sql, libsql::params_from_iter(params)).await?;
  Ok(())
}

/// Delete the `iteration_tasks` rows of many iterations in one statement.
///
/// # Arguments
///
/// * `conn` - the connection, inside the caller's transaction
/// * `iteration_ids` - the iterations' full ids
///
/// # Returns
///
/// `Ok(())` once the statement has run, or the database error.
async fn delete_iteration_tasks_for_iterations(conn: &Connection, iteration_ids: &[String]) -> Result<(), Error> {
  let holders = placeholders(1, iteration_ids.len());
  let sql = format!("DELETE FROM iteration_tasks WHERE iteration_id IN ({holders})");
  let params: Vec<Value> = iteration_ids.iter().map(|id| Value::from(id.clone())).collect();
  conn.execute(&sql, libsql::params_from_iter(params)).await?;
  Ok(())
}

/// Delete the `iteration_tasks` rows of many tasks in one statement.
///
/// # Arguments
///
/// * `conn` - the connection, inside the caller's transaction
/// * `task_ids` - the tasks' full ids
///
/// # Returns
///
/// `Ok(())` once the statement has run, or the database error.
async fn delete_iteration_tasks_for_tasks(conn: &Connection, task_ids: &[String]) -> Result<(), Error> {
  let holders = placeholders(1, task_ids.len());
  let sql = format!("DELETE FROM iteration_tasks WHERE task_id IN ({holders})");
  let params: Vec<Value> = task_ids.iter().map(|id| Value::from(id.clone())).collect();
  conn.execute(&sql, libsql::params_from_iter(params)).await?;
  Ok(())
}

/// Delete the notes of many entities in one statement.
///
/// # Arguments
///
/// * `conn` - the connection, inside the caller's transaction
/// * `entity_type` - the type of every entity
/// * `entity_ids` - the entities' full ids
/// * `any` - whether any notes were captured; when false nothing runs
///
/// # Returns
///
/// `Ok(())` once the statement has run or been skipped, or the database error.
async fn delete_notes_many(
  conn: &Connection,
  entity_type: &str,
  entity_ids: &[String],
  any: bool,
) -> Result<(), Error> {
  if !any {
    return Ok(());
  }
  let holders = placeholders(2, entity_ids.len());
  let sql = format!("DELETE FROM notes WHERE entity_type = ?1 AND entity_id IN ({holders})");
  let mut params: Vec<Value> = Vec::with_capacity(entity_ids.len() + 1);
  params.push(Value::from(entity_type.to_string()));
  for id in entity_ids {
    params.push(Value::from(id.clone()));
  }
  conn.execute(&sql, libsql::params_from_iter(params)).await?;
  Ok(())
}

/// Delete many entities' own rows in one statement.
///
/// # Arguments
///
/// * `conn` - the connection, inside the caller's transaction
/// * `entity_type` - picks the table: `artifacts`, `iterations` or `tasks`
/// * `entity_ids` - the entities' full ids
///
/// # Returns
///
/// `Ok(())` once the statement has run, or the database error.
async fn delete_parents_many(conn: &Connection, entity_type: EntityType, entity_ids: &[String]) -> Result<(), Error> {
  let table = match entity_type {
    EntityType::Artifact => "artifacts",
    EntityType::Iteration => "iterations",
    EntityType::Task => "tasks",
  };
  let holders = placeholders(1, entity_ids.len());
  let sql = format!("DELETE FROM {table} WHERE id IN ({holders})");
  let params: Vec<Value> = entity_ids.iter().map(|id| Value::from(id.clone())).collect();
  conn.execute(&sql, libsql::params_from_iter(params)).await?;
  Ok(())
}

/// Delete the relationships of many entities, in either direction, in one statement.
///
/// # Arguments
///
/// * `conn` - the connection, inside the caller's transaction
/// * `entity_type` - the type of every entity
/// * `entity_ids` - the entities' full ids, matched as source or target
/// * `any` - whether any relationships were captured; when false nothing runs
///
/// # Returns
///
/// `Ok(())` once the statement has run or been skipped, or the database error.
async fn delete_relationships_many(
  conn: &Connection,
  entity_type: &str,
  entity_ids: &[String],
  any: bool,
) -> Result<(), Error> {
  if !any {
    return Ok(());
  }
  let source_holders = placeholders(2, entity_ids.len());
  let target_holders = placeholders(2 + entity_ids.len(), entity_ids.len());
  let sql = format!(
    "DELETE FROM relationships \
      WHERE (source_type = ?1 AND source_id IN ({source_holders})) \
      OR (target_type = ?1 AND target_id IN ({target_holders}))"
  );
  let mut params: Vec<Value> = Vec::with_capacity(entity_ids.len() * 2 + 1);
  params.push(Value::from(entity_type.to_string()));
  for id in entity_ids {
    params.push(Value::from(id.clone()));
  }
  for id in entity_ids {
    params.push(Value::from(id.clone()));
  }
  conn.execute(&sql, libsql::params_from_iter(params)).await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use tempfile::TempDir;

  use super::*;
  use crate::store::{
    self, Db,
    model::{
      Project,
      primitives::{Id, RelationshipType},
    },
    repo::{
      artifact as artifact_repo, iteration as iteration_repo, note as note_repo, relationship as relationship_repo,
      tag as tag_repo, task as task_repo, transaction as transaction_repo,
    },
  };

  async fn setup() -> (Arc<Db>, libsql::Connection, TempDir, Id) {
    let (store, tmp) = store::open_temp().await.unwrap();
    let conn = store.connect().await.unwrap();
    let project = Project::new("/tmp/delete-test".into());
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

  async fn row_count(conn: &libsql::Connection, sql: &str, params: &[libsql::Value]) -> i64 {
    let mut rows = conn.query(sql, params.to_vec()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    row.get(0).unwrap()
  }

  mod captured_child {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn it_round_trips_entity_tag_payload() {
      let child = CapturedChild::EntityTag {
        entity_id: "task-1".into(),
        entity_type: "task".into(),
        tag_id: "tag-1".into(),
        created_at: "2025-01-01T00:00:00Z".into(),
      };

      assert_eq!(child.table(), "entity_tags");
      assert_eq!(child.audit_row_id(), "task:task-1:tag-1");
      assert_eq!(
        child.to_audit_payload(),
        json!({
          "entity_id": "task-1",
          "entity_type": "task",
          "tag_id": "tag-1",
          "created_at": "2025-01-01T00:00:00Z",
        })
      );
    }

    #[test]
    fn it_round_trips_iteration_task_payload() {
      let child = CapturedChild::IterationTask {
        iteration_id: "it-1".into(),
        task_id: "t-1".into(),
        phase: 3,
        created_at: "2025-01-01T00:00:00Z".into(),
      };

      assert_eq!(child.table(), "iteration_tasks");
      assert_eq!(child.audit_row_id(), "it-1:t-1");
      assert_eq!(
        child.to_audit_payload(),
        json!({
          "iteration_id": "it-1",
          "task_id": "t-1",
          "phase": 3,
          "created_at": "2025-01-01T00:00:00Z",
        })
      );
    }

    #[test]
    fn it_round_trips_note_payload_with_null_author() {
      let child = CapturedChild::Note {
        id: "n-1".into(),
        entity_id: "task-1".into(),
        entity_type: "task".into(),
        author_id: None,
        body: "hello".into(),
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
      };

      assert_eq!(child.table(), "notes");
      assert_eq!(child.audit_row_id(), "n-1");
      assert_eq!(
        child.to_audit_payload(),
        json!({
          "id": "n-1",
          "entity_id": "task-1",
          "entity_type": "task",
          "author_id": null,
          "body": "hello",
          "created_at": "2025-01-01T00:00:00Z",
          "updated_at": "2025-01-01T00:00:00Z",
        })
      );
    }

    #[test]
    fn it_round_trips_relationship_payload() {
      let child = CapturedChild::Relationship {
        id: "r-1".into(),
        rel_type: "blocked-by".into(),
        source_id: "task-1".into(),
        source_type: "task".into(),
        target_id: "task-2".into(),
        target_type: "task".into(),
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
      };

      assert_eq!(child.table(), "relationships");
      assert_eq!(child.audit_row_id(), "r-1");
      assert_eq!(
        child.to_audit_payload(),
        json!({
          "id": "r-1",
          "rel_type": "blocked-by",
          "source_id": "task-1",
          "source_type": "task",
          "target_id": "task-2",
          "target_type": "task",
          "created_at": "2025-01-01T00:00:00Z",
          "updated_at": "2025-01-01T00:00:00Z",
        })
      );
    }
  }

  mod delete_with_cascade_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascades_a_task_with_notes_tags_relationships_and_iteration_tasks() {
      let (_store, conn, _tmp, pid) = setup().await;

      let task = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "Target".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let other_task = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "Other".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let iteration = iteration_repo::create(
        &conn,
        &pid,
        &crate::store::model::iteration::New {
          title: "Sprint".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration_repo::add_task(&conn, iteration.id(), task.id(), 1)
        .await
        .unwrap();

      note_repo::create(
        &conn,
        EntityType::Task,
        task.id(),
        &crate::store::model::note::New {
          body: "note body".into(),
          author_id: None,
        },
      )
      .await
      .unwrap();
      tag_repo::attach(&conn, EntityType::Task, task.id(), "blocker")
        .await
        .unwrap();
      relationship_repo::create(
        &conn,
        RelationshipType::BlockedBy,
        EntityType::Task,
        task.id(),
        EntityType::Task,
        other_task.id(),
      )
      .await
      .unwrap();

      let tx = transaction_repo::begin(&conn, &pid, "task delete").await.unwrap();
      let report = delete_with_cascade(&conn, tx.id(), EntityType::Task, task.id())
        .await
        .unwrap();

      assert_eq!(report.notes, 1);
      assert_eq!(report.tags, 1);
      assert_eq!(report.relationships, 1);
      assert_eq!(report.iteration_tasks, 1);

      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM tasks WHERE id = ?1",
          &[task.id().to_string().into()]
        )
        .await,
        0
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM notes WHERE entity_type = 'task' AND entity_id = ?1",
          &[task.id().to_string().into()],
        )
        .await,
        0
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM entity_tags WHERE entity_type = 'task' AND entity_id = ?1",
          &[task.id().to_string().into()],
        )
        .await,
        0
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM relationships WHERE source_id = ?1 OR target_id = ?1",
          &[task.id().to_string().into()],
        )
        .await,
        0
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM iteration_tasks WHERE task_id = ?1",
          &[task.id().to_string().into()],
        )
        .await,
        0
      );
    }

    #[tokio::test]
    async fn it_cascades_an_artifact_with_notes_tags_and_relationships() {
      let (_store, conn, _tmp, pid) = setup().await;

      let artifact = artifact_repo::create(
        &conn,
        &pid,
        &crate::store::model::artifact::New {
          title: "Spec".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let task = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "Related".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      note_repo::create(
        &conn,
        EntityType::Artifact,
        artifact.id(),
        &crate::store::model::note::New {
          body: "spec note".into(),
          author_id: None,
        },
      )
      .await
      .unwrap();
      tag_repo::attach(&conn, EntityType::Artifact, artifact.id(), "design")
        .await
        .unwrap();
      relationship_repo::create(
        &conn,
        RelationshipType::RelatesTo,
        EntityType::Task,
        task.id(),
        EntityType::Artifact,
        artifact.id(),
      )
      .await
      .unwrap();

      let tx = transaction_repo::begin(&conn, &pid, "artifact delete").await.unwrap();
      let report = delete_with_cascade(&conn, tx.id(), EntityType::Artifact, artifact.id())
        .await
        .unwrap();

      assert_eq!(report.notes, 1);
      assert_eq!(report.tags, 1);
      assert_eq!(report.relationships, 1);
      assert_eq!(report.iteration_tasks, 0);

      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM artifacts WHERE id = ?1",
          &[artifact.id().to_string().into()],
        )
        .await,
        0
      );
    }

    #[tokio::test]
    async fn it_cascades_an_iteration_with_iteration_tasks() {
      let (_store, conn, _tmp, pid) = setup().await;

      let iteration = iteration_repo::create(
        &conn,
        &pid,
        &crate::store::model::iteration::New {
          title: "Sprint".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let task_a = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "A".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let task_b = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "B".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration_repo::add_task(&conn, iteration.id(), task_a.id(), 1)
        .await
        .unwrap();
      iteration_repo::add_task(&conn, iteration.id(), task_b.id(), 2)
        .await
        .unwrap();

      let tx = transaction_repo::begin(&conn, &pid, "iteration delete").await.unwrap();
      let report = delete_with_cascade(&conn, tx.id(), EntityType::Iteration, iteration.id())
        .await
        .unwrap();

      assert_eq!(report.iteration_tasks, 2);
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM iterations WHERE id = ?1",
          &[iteration.id().to_string().into()],
        )
        .await,
        0
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM iteration_tasks WHERE iteration_id = ?1",
          &[iteration.id().to_string().into()],
        )
        .await,
        0
      );
      // Sibling tasks must not be touched.
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM tasks WHERE id = ?1",
          &[task_a.id().to_string().into()]
        )
        .await,
        1
      );
    }

    #[tokio::test]
    async fn it_restores_object_metadata_unchanged_on_undo() {
      let (_store, conn, _tmp, pid) = setup().await;

      let task = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "Keeps metadata".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      conn
        .execute(
          "UPDATE tasks SET metadata = '{\"a\":1}' WHERE id = ?1",
          [task.id().to_string()],
        )
        .await
        .unwrap();

      let tx = transaction_repo::begin(&conn, &pid, "task delete").await.unwrap();
      delete_with_cascade(&conn, tx.id(), EntityType::Task, task.id())
        .await
        .unwrap();
      transaction_repo::undo(&conn, tx.id()).await.unwrap();

      let mut rows = conn
        .query("SELECT metadata FROM tasks WHERE id = ?1", [task.id().to_string()])
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      assert_eq!(row.get::<String>(0).unwrap(), "{\"a\":1}");
    }

    #[tokio::test]
    async fn it_is_reversible_via_transaction_undo() {
      let (_store, conn, _tmp, pid) = setup().await;

      let task = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "Reversible".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let iteration = iteration_repo::create(
        &conn,
        &pid,
        &crate::store::model::iteration::New {
          title: "Sprint".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration_repo::add_task(&conn, iteration.id(), task.id(), 3)
        .await
        .unwrap();

      note_repo::create(
        &conn,
        EntityType::Task,
        task.id(),
        &crate::store::model::note::New {
          body: "need".into(),
          author_id: None,
        },
      )
      .await
      .unwrap();
      tag_repo::attach(&conn, EntityType::Task, task.id(), "urgent")
        .await
        .unwrap();

      let tx = transaction_repo::begin(&conn, &pid, "task delete").await.unwrap();
      delete_with_cascade(&conn, tx.id(), EntityType::Task, task.id())
        .await
        .unwrap();

      transaction_repo::undo(&conn, tx.id()).await.unwrap();

      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM tasks WHERE id = ?1",
          &[task.id().to_string().into()]
        )
        .await,
        1
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM notes WHERE entity_type = 'task' AND entity_id = ?1",
          &[task.id().to_string().into()],
        )
        .await,
        1
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM entity_tags WHERE entity_type = 'task' AND entity_id = ?1",
          &[task.id().to_string().into()],
        )
        .await,
        1
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM iteration_tasks WHERE iteration_id = ?1 AND task_id = ?2",
          &[iteration.id().to_string().into(), task.id().to_string().into()],
        )
        .await,
        1
      );
    }
  }

  mod delete_many_with_cascade_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_a_no_op_for_an_empty_slice() {
      let (_store, conn, _tmp, pid) = setup().await;
      let tx = transaction_repo::begin(&conn, &pid, "empty purge").await.unwrap();

      let report = delete_many_with_cascade(&conn, tx.id(), EntityType::Task, &[])
        .await
        .unwrap();

      assert_eq!(report, DeleteReport::default());
    }

    #[tokio::test]
    async fn it_cascades_multiple_tasks_in_a_single_batch() {
      let (_store, conn, _tmp, pid) = setup().await;

      let task_a = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "A".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let task_b = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "B".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let survivor = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "Keep".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let iteration = iteration_repo::create(
        &conn,
        &pid,
        &crate::store::model::iteration::New {
          title: "Sprint".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration_repo::add_task(&conn, iteration.id(), task_a.id(), 1)
        .await
        .unwrap();
      iteration_repo::add_task(&conn, iteration.id(), task_b.id(), 2)
        .await
        .unwrap();

      note_repo::create(
        &conn,
        EntityType::Task,
        task_a.id(),
        &crate::store::model::note::New {
          body: "a".into(),
          author_id: None,
        },
      )
      .await
      .unwrap();
      note_repo::create(
        &conn,
        EntityType::Task,
        task_b.id(),
        &crate::store::model::note::New {
          body: "b".into(),
          author_id: None,
        },
      )
      .await
      .unwrap();
      tag_repo::attach(&conn, EntityType::Task, task_a.id(), "urgent")
        .await
        .unwrap();
      tag_repo::attach(&conn, EntityType::Task, task_b.id(), "blocker")
        .await
        .unwrap();
      relationship_repo::create(
        &conn,
        RelationshipType::BlockedBy,
        EntityType::Task,
        task_a.id(),
        EntityType::Task,
        survivor.id(),
      )
      .await
      .unwrap();

      let tx = transaction_repo::begin(&conn, &pid, "batch delete").await.unwrap();
      let ids: Vec<&Id> = vec![task_a.id(), task_b.id()];
      let report = delete_many_with_cascade(&conn, tx.id(), EntityType::Task, &ids)
        .await
        .unwrap();

      assert_eq!(report.notes, 2);
      assert_eq!(report.tags, 2);
      assert_eq!(report.relationships, 1);
      assert_eq!(report.iteration_tasks, 2);

      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM tasks WHERE id IN (?1, ?2)",
          &[task_a.id().to_string().into(), task_b.id().to_string().into()],
        )
        .await,
        0
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM tasks WHERE id = ?1",
          &[survivor.id().to_string().into()],
        )
        .await,
        1,
        "sibling task must not be touched"
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM iteration_tasks WHERE iteration_id = ?1",
          &[iteration.id().to_string().into()],
        )
        .await,
        0
      );
    }

    #[tokio::test]
    async fn it_is_reversible_via_transaction_undo() {
      let (_store, conn, _tmp, pid) = setup().await;

      let task_a = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "A".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let task_b = task_repo::create(
        &conn,
        &pid,
        &crate::store::model::task::New {
          title: "B".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let iteration = iteration_repo::create(
        &conn,
        &pid,
        &crate::store::model::iteration::New {
          title: "Sprint".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      iteration_repo::add_task(&conn, iteration.id(), task_a.id(), 1)
        .await
        .unwrap();
      note_repo::create(
        &conn,
        EntityType::Task,
        task_a.id(),
        &crate::store::model::note::New {
          body: "hello".into(),
          author_id: None,
        },
      )
      .await
      .unwrap();
      tag_repo::attach(&conn, EntityType::Task, task_b.id(), "urgent")
        .await
        .unwrap();

      let tx = transaction_repo::begin(&conn, &pid, "batch purge").await.unwrap();
      let ids: Vec<&Id> = vec![task_a.id(), task_b.id()];
      delete_many_with_cascade(&conn, tx.id(), EntityType::Task, &ids)
        .await
        .unwrap();

      transaction_repo::undo(&conn, tx.id()).await.unwrap();

      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM tasks WHERE id IN (?1, ?2)",
          &[task_a.id().to_string().into(), task_b.id().to_string().into()],
        )
        .await,
        2
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM notes WHERE entity_type = 'task' AND entity_id = ?1",
          &[task_a.id().to_string().into()],
        )
        .await,
        1
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM entity_tags WHERE entity_type = 'task' AND entity_id = ?1",
          &[task_b.id().to_string().into()],
        )
        .await,
        1
      );
      assert_eq!(
        row_count(
          &conn,
          "SELECT COUNT(*) FROM iteration_tasks WHERE iteration_id = ?1 AND task_id = ?2",
          &[iteration.id().to_string().into(), task_a.id().to_string().into()],
        )
        .await,
        1
      );
    }
  }
}
