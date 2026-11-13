//! Undo journal: transactions, the row-level events recorded inside them, and their replay.
//!
//! Each undoable command opens a transaction row and records one event per changed row, with the
//! row's state before the change. [`undo`] replays those events in reverse to restore that state.

use chrono::Utc;
use libsql::{Connection, Value};
use serde_json::Value as JsonValue;

use crate::store::{
  Error,
  model::{
    primitives::Id,
    transaction::{Event, Model},
  },
  repo::resolve::Table,
};

/// The columns of `transactions` in the order that the transaction model's `TryFrom<Row>` reads them.
const SELECT_COLUMNS: &str = "id, project_id, author_id, command, undone_at, created_at";

/// Begin a new transaction for the given command, with no author.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project the transaction belongs to
/// * `command` - the command text recorded on the transaction, which [`undo`] returns
///
/// # Returns
///
/// The inserted transaction as read back from the database, or an error when the insert or the read fails.
pub async fn begin(conn: &Connection, project_id: &Id, command: &str) -> Result<Model, Error> {
  log::debug!("repo::transaction::begin");
  begin_with_author(conn, project_id, command, None).await
}

/// Begin a new transaction attributed to a specific author.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project the transaction belongs to
/// * `command` - the command text recorded on the transaction, which [`undo`] returns
/// * `author_id` - the author to record, or `None` to store `NULL`
///
/// # Returns
///
/// The inserted transaction as read back from the database, or an error when the insert or the read fails.
pub async fn begin_with_author(
  conn: &Connection,
  project_id: &Id,
  command: &str,
  author_id: Option<&Id>,
) -> Result<Model, Error> {
  log::debug!("repo::transaction::begin_with_author");
  let id = Id::new();
  let now = Utc::now();
  let author: Value = match author_id {
    Some(a) => Value::from(a.to_string()),
    None => Value::Null,
  };
  conn
    .execute(
      "INSERT INTO transactions (id, project_id, author_id, command, created_at) \
        VALUES (?1, ?2, ?3, ?4, ?5)",
      libsql::params![
        id.to_string(),
        project_id.to_string(),
        author,
        command.to_string(),
        now.to_rfc3339(),
      ],
    )
    .await?;

  find_by_id(conn, id)
    .await?
    .ok_or_else(|| Error::InvalidValue("transaction not found after insert".into()))
}

/// Find a transaction by its ID.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `id` - the full transaction id
///
/// # Returns
///
/// The transaction, `None` when no row has that id, or an error when the query or row decoding fails.
pub async fn find_by_id(conn: &Connection, id: impl Into<Id>) -> Result<Option<Model>, Error> {
  log::debug!("repo::transaction::find_by_id");
  let id = id.into();
  let mut rows = conn
    .query(
      &format!("SELECT {SELECT_COLUMNS} FROM transactions WHERE id = ?1"),
      [id.to_string()],
    )
    .await?;

  match rows.next().await? {
    Some(row) => Ok(Some(Model::try_from(row)?)),
    None => Ok(None),
  }
}

/// Return the most recent non-undone transaction for a project.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project to search
///
/// # Returns
///
/// The transaction, newest by `created_at`; `None` when there is none, or an error when the query or row
/// decoding fails.
pub async fn latest_undoable(conn: &Connection, project_id: &Id) -> Result<Option<Model>, Error> {
  log::debug!("repo::transaction::latest_undoable");
  let mut rows = conn
    .query(
      &format!(
        "SELECT {SELECT_COLUMNS} FROM transactions \
          WHERE project_id = ?1 AND undone_at IS NULL \
          ORDER BY created_at DESC LIMIT 1"
      ),
      [project_id.to_string()],
    )
    .await?;

  match rows.next().await? {
    Some(row) => Ok(Some(Model::try_from(row)?)),
    None => Ok(None),
  }
}

/// Return the N most recent non-undone transactions for a project.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project to search
/// * `n` - the most transactions to return
///
/// # Returns
///
/// Up to `n` transactions, newest first, or an error when the query or row decoding fails.
pub async fn latest_undoable_n(conn: &Connection, project_id: &Id, n: u32) -> Result<Vec<Model>, Error> {
  log::debug!("repo::transaction::latest_undoable_n");
  let mut rows = conn
    .query(
      &format!(
        "SELECT {SELECT_COLUMNS} FROM transactions \
          WHERE project_id = ?1 AND undone_at IS NULL \
          ORDER BY created_at DESC LIMIT ?2"
      ),
      libsql::params![project_id.to_string(), n],
    )
    .await?;

  let mut results = Vec::new();
  while let Some(row) = rows.next().await? {
    results.push(Model::try_from(row)?);
  }
  Ok(results)
}

/// Record a change event within a transaction.
///
/// This records the row-level audit metadata needed for undo replay and leaves
/// the timeline-facing semantic fields `NULL`. Use [`record_semantic_event`]
/// instead when the mutation maps to a user-facing activity entry
/// (create/status-change/phase-change/priority-change/archive/complete/cancel).
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `transaction_id` - the transaction the event belongs to
/// * `table_name` - the table of the changed row, as its SQL name
/// * `row_id` - the primary key of the changed row
/// * `event_type` - `created`, `modified` or `deleted`; [`undo`] ignores any other value
/// * `before_data` - the row as JSON before the change, or `None` for a created row; a created row in a join table
///   carries its key columns instead, as [`undo`] needs them
///
/// # Returns
///
/// `Ok(())` once the event row is inserted, or an error when the insert fails.
pub async fn record_event(
  conn: &Connection,
  transaction_id: &Id,
  table_name: &str,
  row_id: &str,
  event_type: &str,
  before_data: Option<&JsonValue>,
) -> Result<(), Error> {
  log::debug!("repo::transaction::record_event");
  record_semantic_event(
    conn,
    transaction_id,
    table_name,
    row_id,
    event_type,
    before_data,
    None,
    None,
    None,
  )
  .await
}

/// Record a change event with semantic timeline metadata.
///
/// `semantic_type`, `old_value`, and `new_value` feed the unified activity
/// timeline. They should be populated for user-facing mutations and left
/// `None` for internal or free-form edits that have no human-readable
/// timeline entry (in which case the plain [`record_event`] is sufficient).
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `transaction_id` - the transaction the event belongs to
/// * `table_name` - the table of the changed row, as its SQL name
/// * `row_id` - the primary key of the changed row
/// * `event_type` - `created`, `modified` or `deleted`; [`undo`] ignores any other value
/// * `before_data` - the row as JSON before the change, or `None` for a created row; a created row in a join table
///   carries its key columns instead, as [`undo`] needs them
/// * `semantic_type` - the activity kind
/// * `old_value` - the value before the change
/// * `new_value` - the value after the change
///
/// # Returns
///
/// `Ok(())` once the event row is inserted, or an error when the insert fails.
#[allow(clippy::too_many_arguments)]
pub async fn record_semantic_event(
  conn: &Connection,
  transaction_id: &Id,
  table_name: &str,
  row_id: &str,
  event_type: &str,
  before_data: Option<&JsonValue>,
  semantic_type: Option<&str>,
  old_value: Option<&str>,
  new_value: Option<&str>,
) -> Result<(), Error> {
  log::debug!("repo::transaction::record_semantic_event");
  let id = Id::new();
  let before: Value = match before_data {
    Some(d) => Value::from(d.to_string()),
    None => Value::Null,
  };
  let semantic: Value = match semantic_type {
    Some(s) => Value::from(s.to_string()),
    None => Value::Null,
  };
  let old: Value = match old_value {
    Some(v) => Value::from(v.to_string()),
    None => Value::Null,
  };
  let new: Value = match new_value {
    Some(v) => Value::from(v.to_string()),
    None => Value::Null,
  };
  conn
    .execute(
      "INSERT INTO transaction_events \
        (id, transaction_id, before_data, event_type, row_id, table_name, semantic_type, old_value, new_value, created_at) \
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
      libsql::params![
        id.to_string(),
        transaction_id.to_string(),
        before,
        event_type.to_string(),
        row_id.to_string(),
        table_name.to_string(),
        semantic,
        old,
        new,
        Utc::now().to_rfc3339(),
      ],
    )
    .await?;
  Ok(())
}

/// A semantic event entry for the activity timeline, joined with its parent
/// transaction's author and creation timestamp.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticEvent {
  /// Author of the parent transaction, if any.
  pub author_id: Option<Id>,
  /// When the event was recorded.
  pub created_at: chrono::DateTime<Utc>,
  /// Unique identifier of the event row.
  pub id: Id,
  /// Post-change value, when applicable.
  pub new_value: Option<String>,
  /// Pre-change value, when applicable.
  pub old_value: Option<String>,
  /// Primary key of the row the event refers to.
  pub row_id: String,
  /// User-facing activity kind (e.g. `created`, `status-change`).
  pub semantic_type: String,
  /// Name of the table the row belongs to.
  pub table_name: String,
}

/// Return all semantic events for a given row, ordered by creation time ascending.
///
/// Filters out events with `semantic_type IS NULL`, so free-form edits
/// never appear in the activity timeline. Events of undone transactions are left out too.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `table_name` - the table of the row, as its SQL name
/// * `row_id` - the primary key of the row
///
/// # Returns
///
/// The events, each with its transaction's author, or an error when the query fails or a stored id or
/// timestamp does not parse.
pub async fn semantic_events_for_row(
  conn: &Connection,
  table_name: &str,
  row_id: &Id,
) -> Result<Vec<SemanticEvent>, Error> {
  log::debug!("repo::transaction::semantic_events_for_row");
  let mut rows = conn
    .query(
      "SELECT te.id, te.row_id, te.table_name, te.semantic_type, te.old_value, te.new_value, \
        te.created_at, t.author_id \
        FROM transaction_events te \
        JOIN transactions t ON t.id = te.transaction_id \
        WHERE te.table_name = ?1 AND te.row_id = ?2 \
          AND te.semantic_type IS NOT NULL \
          AND t.undone_at IS NULL \
        ORDER BY te.created_at ASC",
      libsql::params![table_name.to_string(), row_id.to_string()],
    )
    .await?;

  let mut results = Vec::new();
  while let Some(row) = rows.next().await? {
    let id: String = row.get(0)?;
    let row_id: String = row.get(1)?;
    let table_name: String = row.get(2)?;
    let semantic_type: String = row.get(3)?;
    let old_value: Option<String> = row.get(4)?;
    let new_value: Option<String> = row.get(5)?;
    let created_at: String = row.get(6)?;
    let author_id: Option<String> = row.get(7)?;

    let id: Id = id.parse().map_err(Error::InvalidValue)?;
    let author_id = author_id
      .map(|s| s.parse::<Id>())
      .transpose()
      .map_err(Error::InvalidValue)?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
      .map(|dt| dt.with_timezone(&Utc))
      .map_err(|e| Error::InvalidValue(e.to_string()))?;

    results.push(SemanticEvent {
      author_id,
      created_at,
      id,
      new_value,
      old_value,
      row_id,
      semantic_type,
      table_name,
    });
  }
  Ok(results)
}

/// Undo a transaction by replaying its events in reverse order of insertion. The order does not use `created_at`,
/// so a clock that steps back during a command cannot swap two of its events.
///
/// A `created` event deletes the row whose `id` is the event's `row_id`, a `modified` event writes back the
/// columns of `before_data` other than `id`, and a `deleted` event re-inserts `before_data` with
/// `INSERT OR IGNORE`. The transaction is then marked undone. `entity_tags` and `iteration_tasks` have no `id`
/// column, so a `created` event on them carries the row's key columns ([`Table::key_columns`]) in `before_data`,
/// and undo deletes the row with that key. Such an event recorded without its key fails with
/// [`Error::InvalidValue`].
///
/// In a `modified` event, a JSON null in `before_data` writes back SQL `NULL`, except in a JSON column such as
/// `metadata`, where it writes back the text `null`. A `deleted` event's `before_data` is the raw row, so its
/// values are written back as stored.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `transaction_id` - the transaction to undo
///
/// # Returns
///
/// The command text of the undone transaction, [`Error::NothingToUndo`] when no transaction has that id, or
/// [`Error::InvalidValue`] when an event names an unknown table or column, or a database error.
pub async fn undo(conn: &Connection, transaction_id: &Id) -> Result<String, Error> {
  log::debug!("repo::transaction::undo");
  // Get the transaction
  let tx = find_by_id(conn, transaction_id.clone())
    .await?
    .ok_or(Error::NothingToUndo)?;

  // Get all events in reverse order
  let mut rows = conn
    .query(
      "SELECT id, transaction_id, before_data, event_type, row_id, table_name, \
        semantic_type, old_value, new_value, created_at \
        FROM transaction_events WHERE transaction_id = ?1 ORDER BY rowid DESC",
      [transaction_id.to_string()],
    )
    .await?;

  let mut events = Vec::new();
  while let Some(row) = rows.next().await? {
    events.push(Event::try_from(row)?);
  }

  // Replay each event
  for event in &events {
    // Resolve the stored `table_name` through the closed allowlist. A row with
    // an unrecognized identifier indicates either a schema-migration gap or a
    // tampered DB; either way, we refuse to interpolate it into dynamic SQL.
    let table = Table::from_sql_ident(event.table_name())
      .ok_or_else(|| Error::InvalidValue(format!("unknown table in transaction_events: {}", event.table_name())))?;
    let ident = table.as_sql_ident();

    match event.event_type().as_str() {
      "created" => {
        // Undo a create by deleting the row. A join table has no `id`, so its
        // event carries the row's key columns in before_data.
        let keys = table.key_columns();
        if keys == ["id"] {
          let sql = format!("DELETE FROM {ident} WHERE id = ?1");
          conn.execute(&sql, [event.row_id().to_string()]).await?;
        } else {
          let key = event.before_data().as_ref().and_then(|before| before.as_object());
          let mut conds = Vec::new();
          let mut params: Vec<Value> = Vec::new();
          for column in keys {
            let value = key
              .and_then(|k| k.get(*column))
              .and_then(|v| v.as_str())
              .ok_or_else(|| {
                Error::InvalidValue(format!(
                  "cannot undo a created row in {ident}: the event was recorded without its key column {column}"
                ))
              })?;
            params.push(Value::from(value.to_string()));
            conds.push(format!("{column} = ?{}", params.len()));
          }
          let sql = format!("DELETE FROM {ident} WHERE {}", conds.join(" AND "));
          conn.execute(&sql, libsql::params_from_iter(params)).await?;
        }
      }
      "modified" => {
        // Undo a modify by restoring before_data
        if let Some(before) = event.before_data()
          && let Some(obj) = before.as_object()
        {
          // Each column gets its own parameter, and a null in before_data
          // restores SQL NULL, so the columns after it keep their own values.
          let mut sets = Vec::new();
          let mut params: Vec<Value> = Vec::new();
          for (key, val) in obj {
            if key == "id" {
              continue;
            }
            if !table.has_column(key) {
              return Err(Error::InvalidValue(format!(
                "unknown column in before_data for table {ident}: {key}"
              )));
            }
            params.push(json_to_sql(key, val));
            sets.push(format!("{key} = ?{}", params.len()));
          }
          if !sets.is_empty() {
            params.push(Value::from(event.row_id().to_string()));
            let sql = format!("UPDATE {ident} SET {} WHERE id = ?{}", sets.join(", "), params.len());
            conn.execute(&sql, libsql::params_from_iter(params)).await?;
          }
        }
      }
      "deleted" => {
        // Undo a delete by re-inserting from before_data. NULL values must be
        // passed through as SQL NULL rather than an empty string so that
        // nullable FK columns (e.g. `tasks.assigned_to`) do not spuriously
        // violate constraints on replay.
        if let Some(before) = event.before_data()
          && let Some(obj) = before.as_object()
        {
          for key in obj.keys() {
            if !table.has_column(key) {
              return Err(Error::InvalidValue(format!(
                "unknown column in before_data for table {ident}: {key}"
              )));
            }
          }
          let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
          let placeholders: Vec<String> = (1..=keys.len()).map(|i| format!("?{i}")).collect();
          // A deleted event's before_data is the raw row, so `metadata` is already its stored JSON text.
          let values: Vec<Value> = obj.values().map(raw_row_to_sql).collect();
          let sql = format!(
            "INSERT OR IGNORE INTO {ident} ({}) VALUES ({})",
            keys.join(", "),
            placeholders.join(", ")
          );
          conn.execute(&sql, libsql::params_from_iter(values)).await?;
        }
      }
      _ => {}
    }
  }

  // Mark the transaction as undone
  conn
    .execute(
      "UPDATE transactions SET undone_at = ?1 WHERE id = ?2",
      [Utc::now().to_rfc3339(), transaction_id.to_string()],
    )
    .await?;

  Ok(tx.command().to_string())
}

/// Columns that store JSON as text. Their value is written back as JSON text, even when it is `null`.
const JSON_COLUMNS: &[&str] = &["metadata"];

/// Converts one value of a `modified` event's `before_data` into the SQL value that restores it.
///
/// A `modified` event's `before_data` is the serialized model, so a JSON column such as `metadata` holds a JSON
/// value, not its stored text.
///
/// # Arguments
///
/// * `column` - the column the value belongs to
/// * `value` - the JSON value of that column
///
/// # Returns
///
/// For a JSON column, the JSON text of the value, so a `null` comes back as the text `null` and not as SQL `NULL`.
/// For any other column, the same value as [`raw_row_to_sql`].
fn json_to_sql(column: &str, value: &JsonValue) -> Value {
  if JSON_COLUMNS.contains(&column) {
    return Value::from(value.to_string());
  }
  raw_row_to_sql(value)
}

/// Converts one value of a raw row, as a `deleted` event stores it, into the SQL value that restores it.
///
/// # Arguments
///
/// * `value` - the JSON value of one column of the row
///
/// # Returns
///
/// SQL `NULL` for a JSON null, the text for a JSON string, and the JSON text of any other value.
fn raw_row_to_sql(value: &JsonValue) -> Value {
  match value {
    JsonValue::Null => Value::Null,
    JsonValue::String(s) => Value::from(s.clone()),
    other => Value::from(other.to_string()),
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use tempfile::TempDir;

  use super::*;
  use crate::store::{
    self, Db,
    model::{Project, primitives::EntityType},
  };

  async fn setup() -> (Arc<Db>, Connection, TempDir, Id) {
    let (store, tmp) = store::open_temp().await.unwrap();
    let conn = store.connect().await.unwrap();
    let project = Project::new("/tmp/tx-test".into());
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

  mod begin_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_a_transaction() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task create").await.unwrap();

      assert_eq!(tx.command(), "task create");
      assert!(tx.undone_at().is_none());
    }
  }

  mod latest_undoable_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_most_recent() {
      let (_store, conn, _tmp, pid) = setup().await;

      begin(&conn, &pid, "first").await.unwrap();
      let second = begin(&conn, &pid, "second").await.unwrap();

      let latest = latest_undoable(&conn, &pid).await.unwrap().unwrap();

      assert_eq!(latest.id(), second.id());
    }

    #[tokio::test]
    async fn it_returns_none_when_empty() {
      let (_store, conn, _tmp, pid) = setup().await;

      let latest = latest_undoable(&conn, &pid).await.unwrap();

      assert!(latest.is_none());
    }
  }

  mod latest_undoable_n_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_none() {
      let (_store, conn, _tmp, pid) = setup().await;

      let results = latest_undoable_n(&conn, &pid, 3).await.unwrap();

      assert!(results.is_empty());
    }

    #[tokio::test]
    async fn it_returns_fewer_when_not_enough() {
      let (_store, conn, _tmp, pid) = setup().await;

      begin(&conn, &pid, "only one").await.unwrap();

      let results = latest_undoable_n(&conn, &pid, 5).await.unwrap();

      assert_eq!(results.len(), 1);
      assert_eq!(results[0].command(), "only one");
    }

    #[tokio::test]
    async fn it_returns_n_most_recent() {
      let (_store, conn, _tmp, pid) = setup().await;

      begin(&conn, &pid, "first").await.unwrap();
      begin(&conn, &pid, "second").await.unwrap();
      begin(&conn, &pid, "third").await.unwrap();

      let results = latest_undoable_n(&conn, &pid, 2).await.unwrap();

      assert_eq!(results.len(), 2);
      assert_eq!(results[0].command(), "third");
      assert_eq!(results[1].command(), "second");
    }
  }

  mod record_event_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn semantic_row(conn: &Connection, tx_id: &Id) -> (Option<String>, Option<String>, Option<String>) {
      let mut rows = conn
        .query(
          "SELECT semantic_type, old_value, new_value FROM transaction_events WHERE transaction_id = ?1",
          [tx_id.to_string()],
        )
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      (row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap())
    }

    #[tokio::test]
    async fn it_leaves_semantic_fields_null_for_plain_record_event() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task update").await.unwrap();
      record_event(&conn, tx.id(), "tasks", "abc", "modified", None)
        .await
        .unwrap();

      let (semantic, old, new) = semantic_row(&conn, tx.id()).await;

      assert_eq!(semantic, None);
      assert_eq!(old, None);
      assert_eq!(new, None);
    }
  }

  mod record_semantic_event_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn semantic_row(conn: &Connection, tx_id: &Id) -> (Option<String>, Option<String>, Option<String>) {
      let mut rows = conn
        .query(
          "SELECT semantic_type, old_value, new_value FROM transaction_events WHERE transaction_id = ?1",
          [tx_id.to_string()],
        )
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      (row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap())
    }

    #[tokio::test]
    async fn it_persists_archived_semantic_type() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "artifact archive").await.unwrap();
      record_semantic_event(
        &conn,
        tx.id(),
        "artifacts",
        "abc",
        "modified",
        None,
        Some("archived"),
        None,
        None,
      )
      .await
      .unwrap();

      let (semantic, _, _) = semantic_row(&conn, tx.id()).await;

      assert_eq!(semantic.as_deref(), Some("archived"));
    }

    #[tokio::test]
    async fn it_persists_cancelled_semantic_type() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task cancel").await.unwrap();
      record_semantic_event(
        &conn,
        tx.id(),
        "tasks",
        "abc",
        "modified",
        None,
        Some("cancelled"),
        Some("open"),
        Some("cancelled"),
      )
      .await
      .unwrap();

      let (semantic, _, _) = semantic_row(&conn, tx.id()).await;

      assert_eq!(semantic.as_deref(), Some("cancelled"));
    }

    #[tokio::test]
    async fn it_persists_completed_semantic_type() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task complete").await.unwrap();
      record_semantic_event(
        &conn,
        tx.id(),
        "tasks",
        "abc",
        "modified",
        None,
        Some("completed"),
        Some("open"),
        Some("done"),
      )
      .await
      .unwrap();

      let (semantic, old, new) = semantic_row(&conn, tx.id()).await;

      assert_eq!(semantic.as_deref(), Some("completed"));
      assert_eq!(old.as_deref(), Some("open"));
      assert_eq!(new.as_deref(), Some("done"));
    }

    #[tokio::test]
    async fn it_persists_created_semantic_type() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task create").await.unwrap();
      record_semantic_event(
        &conn,
        tx.id(),
        "tasks",
        "abc",
        "created",
        None,
        Some("created"),
        None,
        None,
      )
      .await
      .unwrap();

      let (semantic, old, new) = semantic_row(&conn, tx.id()).await;

      assert_eq!(semantic.as_deref(), Some("created"));
      assert_eq!(old, None);
      assert_eq!(new, None);
    }

    #[tokio::test]
    async fn it_persists_phase_change_with_old_and_new_values() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task update").await.unwrap();
      record_semantic_event(
        &conn,
        tx.id(),
        "iteration_tasks",
        "abc",
        "modified",
        None,
        Some("phase-change"),
        Some("1"),
        Some("2"),
      )
      .await
      .unwrap();

      let (semantic, old, new) = semantic_row(&conn, tx.id()).await;

      assert_eq!(semantic.as_deref(), Some("phase-change"));
      assert_eq!(old.as_deref(), Some("1"));
      assert_eq!(new.as_deref(), Some("2"));
    }

    #[tokio::test]
    async fn it_persists_priority_change_with_old_and_new_values() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task update").await.unwrap();
      record_semantic_event(
        &conn,
        tx.id(),
        "tasks",
        "abc",
        "modified",
        None,
        Some("priority-change"),
        Some("2"),
        Some("1"),
      )
      .await
      .unwrap();

      let (semantic, old, new) = semantic_row(&conn, tx.id()).await;

      assert_eq!(semantic.as_deref(), Some("priority-change"));
      assert_eq!(old.as_deref(), Some("2"));
      assert_eq!(new.as_deref(), Some("1"));
    }

    #[tokio::test]
    async fn it_persists_status_change_with_old_and_new_values() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task update").await.unwrap();
      record_semantic_event(
        &conn,
        tx.id(),
        "tasks",
        "abc",
        "modified",
        None,
        Some("status-change"),
        Some("open"),
        Some("in-progress"),
      )
      .await
      .unwrap();

      let (semantic, old, new) = semantic_row(&conn, tx.id()).await;

      assert_eq!(semantic.as_deref(), Some("status-change"));
      assert_eq!(old.as_deref(), Some("open"));
      assert_eq!(new.as_deref(), Some("in-progress"));
    }
  }

  mod undo_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    /// Inserts a task and an iteration, and returns their ids.
    async fn task_and_iteration(conn: &Connection, pid: &Id) -> (Id, Id) {
      let task_id = Id::new();
      let iteration_id = Id::new();
      conn
        .execute(
          "INSERT INTO tasks (id, project_id, title) VALUES (?1, ?2, 'T')",
          [task_id.to_string(), pid.to_string()],
        )
        .await
        .unwrap();
      conn
        .execute(
          "INSERT INTO iterations (id, project_id, title) VALUES (?1, ?2, 'I')",
          [iteration_id.to_string(), pid.to_string()],
        )
        .await
        .unwrap();
      (task_id, iteration_id)
    }

    async fn count(conn: &Connection, sql: &str, params: Vec<String>) -> i64 {
      let mut rows = conn.query(sql, libsql::params_from_iter(params)).await.unwrap();
      rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap()
    }

    #[tokio::test]
    async fn it_undoes_a_new_tag_link_and_keeps_the_same_tag_on_other_entities() {
      let (_store, conn, _tmp, pid) = setup().await;
      let (task_id, other_id) = task_and_iteration(&conn, &pid).await;
      crate::store::repo::tag::attach(&conn, EntityType::Iteration, &other_id, "urgent")
        .await
        .unwrap();

      let tx = begin(&conn, &pid, "task tag").await.unwrap();
      crate::store::repo::tag::attach_recorded(&conn, tx.id(), EntityType::Task, &task_id, "urgent")
        .await
        .unwrap();
      undo(&conn, tx.id()).await.unwrap();

      let sql = "SELECT COUNT(*) FROM entity_tags WHERE entity_id = ?1";
      assert_eq!(count(&conn, sql, vec![task_id.to_string()]).await, 0);
      assert_eq!(count(&conn, sql, vec![other_id.to_string()]).await, 1);
    }

    #[tokio::test]
    async fn it_keeps_a_tag_link_that_was_there_before_the_transaction() {
      let (_store, conn, _tmp, pid) = setup().await;
      let (task_id, _) = task_and_iteration(&conn, &pid).await;
      crate::store::repo::tag::attach(&conn, EntityType::Task, &task_id, "urgent")
        .await
        .unwrap();

      let tx = begin(&conn, &pid, "task tag").await.unwrap();
      crate::store::repo::tag::attach_recorded(&conn, tx.id(), EntityType::Task, &task_id, "urgent")
        .await
        .unwrap();
      undo(&conn, tx.id()).await.unwrap();

      let sql = "SELECT COUNT(*) FROM entity_tags WHERE entity_id = ?1";
      assert_eq!(count(&conn, sql, vec![task_id.to_string()]).await, 1);
    }

    #[tokio::test]
    async fn it_undoes_adding_a_task_to_an_iteration_and_keeps_its_other_iterations() {
      let (_store, conn, _tmp, pid) = setup().await;
      let (task_id, iteration_id) = task_and_iteration(&conn, &pid).await;
      let (_, other_iteration) = task_and_iteration(&conn, &pid).await;
      crate::store::repo::iteration::add_task(&conn, &other_iteration, &task_id, 1)
        .await
        .unwrap();

      let tx = begin(&conn, &pid, "iteration add").await.unwrap();
      crate::store::repo::iteration::add_task_recorded(&conn, tx.id(), &iteration_id, &task_id, 1)
        .await
        .unwrap();
      undo(&conn, tx.id()).await.unwrap();

      let sql = "SELECT COUNT(*) FROM iteration_tasks WHERE iteration_id = ?1 AND task_id = ?2";
      assert_eq!(
        count(&conn, sql, vec![iteration_id.to_string(), task_id.to_string()]).await,
        0
      );
      assert_eq!(
        count(&conn, sql, vec![other_iteration.to_string(), task_id.to_string()]).await,
        1
      );
    }

    #[tokio::test]
    async fn it_keeps_a_task_that_was_in_the_iteration_before_the_transaction() {
      let (_store, conn, _tmp, pid) = setup().await;
      let (task_id, iteration_id) = task_and_iteration(&conn, &pid).await;
      crate::store::repo::iteration::add_task(&conn, &iteration_id, &task_id, 1)
        .await
        .unwrap();

      let tx = begin(&conn, &pid, "iteration add").await.unwrap();
      crate::store::repo::iteration::add_task_recorded(&conn, tx.id(), &iteration_id, &task_id, 2)
        .await
        .unwrap();
      undo(&conn, tx.id()).await.unwrap();

      let sql = "SELECT COUNT(*) FROM iteration_tasks WHERE iteration_id = ?1 AND task_id = ?2";
      assert_eq!(
        count(&conn, sql, vec![iteration_id.to_string(), task_id.to_string()]).await,
        1
      );
    }

    #[tokio::test]
    async fn it_replays_events_in_reverse_insertion_order_even_when_the_clock_stepped_back() {
      let (_store, conn, _tmp, pid) = setup().await;
      let (task_id, _) = task_and_iteration(&conn, &pid).await;
      crate::store::repo::tag::attach(&conn, EntityType::Task, &task_id, "a")
        .await
        .unwrap();

      // A tag update drops the link and adds it again; the clock steps back between the two events.
      let tx = begin(&conn, &pid, "task update").await.unwrap();
      crate::store::repo::tag::detach_all_recorded(&conn, tx.id(), EntityType::Task, &task_id)
        .await
        .unwrap();
      crate::store::repo::tag::attach_recorded(&conn, tx.id(), EntityType::Task, &task_id, "a")
        .await
        .unwrap();
      conn
        .execute(
          "UPDATE transaction_events SET created_at = '2000-01-01T00:00:00+00:00' \
            WHERE transaction_id = ?1 AND event_type = 'created'",
          [tx.id().to_string()],
        )
        .await
        .unwrap();
      undo(&conn, tx.id()).await.unwrap();

      let sql = "SELECT COUNT(*) FROM entity_tags WHERE entity_id = ?1";
      assert_eq!(count(&conn, sql, vec![task_id.to_string()]).await, 1);
    }

    #[tokio::test]
    async fn it_refuses_a_join_table_create_recorded_without_its_key() {
      let (_store, conn, _tmp, pid) = setup().await;

      let tx = begin(&conn, &pid, "task tag").await.unwrap();
      record_event(&conn, tx.id(), "entity_tags", "some-tag-id", "created", None)
        .await
        .unwrap();
      let err = undo(&conn, tx.id()).await.unwrap_err();

      assert!(
        matches!(&err, Error::InvalidValue(m) if m.contains("entity_tags") && m.contains("key")),
        "{err}"
      );
    }

    #[tokio::test]
    async fn it_restores_null_metadata_as_json_text() {
      let (_store, conn, _tmp, pid) = setup().await;

      // Sync import can store a task's metadata as the JSON text `null`; the
      // model then loads it as JSON null. `metadata` is TEXT NOT NULL.
      let task_id = Id::new();
      conn
        .execute(
          "INSERT INTO tasks (id, project_id, title, metadata) VALUES (?1, ?2, 'Task', '{\"a\":1}')",
          [task_id.to_string(), pid.to_string()],
        )
        .await
        .unwrap();

      let tx = begin(&conn, &pid, "task update").await.unwrap();
      let before = serde_json::json!({
        "id": task_id.to_string(),
        "metadata": null,
        "title": "Task",
      });
      record_event(&conn, tx.id(), "tasks", &task_id.to_string(), "modified", Some(&before))
        .await
        .unwrap();

      undo(&conn, tx.id()).await.unwrap();

      let mut rows = conn
        .query("SELECT metadata FROM tasks WHERE id = ?1", [task_id.to_string()])
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      assert_eq!(row.get::<String>(0).unwrap(), "null");
    }

    #[tokio::test]
    async fn it_restores_a_null_column_as_null_and_not_as_the_next_columns_value() {
      let (_store, conn, _tmp, pid) = setup().await;

      let task_id = Id::new();
      conn
        .execute(
          "INSERT INTO tasks (id, project_id, title, description, priority) VALUES (?1, ?2, 'New title', 'new desc', 1)",
          [task_id.to_string(), pid.to_string()],
        )
        .await
        .unwrap();

      // `priority` sorts between `description` and `title`. The null must
      // restore SQL NULL, not take the parameter of the column after it.
      let tx = begin(&conn, &pid, "task update").await.unwrap();
      let before = serde_json::json!({
        "description": "old desc",
        "id": task_id.to_string(),
        "priority": null,
        "title": "Old title",
      });
      record_event(&conn, tx.id(), "tasks", &task_id.to_string(), "modified", Some(&before))
        .await
        .unwrap();

      undo(&conn, tx.id()).await.unwrap();

      let mut rows = conn
        .query(
          "SELECT title, description, priority FROM tasks WHERE id = ?1",
          [task_id.to_string()],
        )
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      assert_eq!(row.get::<String>(0).unwrap(), "Old title");
      assert_eq!(row.get::<String>(1).unwrap(), "old desc");
      assert_eq!(row.get_value(2).unwrap(), Value::Null);
    }

    #[tokio::test]
    async fn it_undoes_a_create_by_deleting() {
      let (_store, conn, _tmp, pid) = setup().await;

      // Create a task
      let task_id = Id::new();
      conn
        .execute(
          "INSERT INTO tasks (id, project_id, title) VALUES (?1, ?2, ?3)",
          [task_id.to_string(), pid.to_string(), "Undo me".to_string()],
        )
        .await
        .unwrap();

      // Record the transaction
      let tx = begin(&conn, &pid, "task create").await.unwrap();
      record_event(&conn, tx.id(), "tasks", &task_id.to_string(), "created", None)
        .await
        .unwrap();

      // Undo it
      let cmd = undo(&conn, tx.id()).await.unwrap();
      assert_eq!(cmd, "task create");

      // Task should be gone
      let mut rows = conn
        .query("SELECT id FROM tasks WHERE id = ?1", [task_id.to_string()])
        .await
        .unwrap();
      assert!(rows.next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_errors_when_before_data_contains_unknown_column_for_deleted_event() {
      let (_store, conn, _tmp, pid) = setup().await;

      // Hand-craft a `deleted` event whose `before_data` names a column that
      // does not exist on `tasks`. The allowlist must reject it before any
      // SQL is rendered.
      let tx = begin(&conn, &pid, "task delete").await.unwrap();
      let bogus = serde_json::json!({
        "id": "abc",
        "title": "Task",
        "not_a_real_column": "value",
      });
      record_event(&conn, tx.id(), "tasks", "abc", "deleted", Some(&bogus))
        .await
        .unwrap();

      let err = undo(&conn, tx.id()).await.unwrap_err();
      let msg = err.to_string();
      assert!(msg.contains("unknown column"), "unexpected error: {msg}");
      assert!(msg.contains("not_a_real_column"), "unexpected error: {msg}");
    }

    #[tokio::test]
    async fn it_errors_when_before_data_contains_unknown_column_for_modified_event() {
      let (_store, conn, _tmp, pid) = setup().await;

      // Insert a task so the row exists even though we expect the replay to
      // bail before issuing any UPDATE.
      let task_id = Id::new();
      conn
        .execute(
          "INSERT INTO tasks (id, project_id, title) VALUES (?1, ?2, ?3)",
          [task_id.to_string(), pid.to_string(), "Task".to_string()],
        )
        .await
        .unwrap();

      let tx = begin(&conn, &pid, "task update").await.unwrap();
      let before = serde_json::json!({
        "id": task_id.to_string(),
        "not_a_real_column": "value",
      });
      record_event(&conn, tx.id(), "tasks", &task_id.to_string(), "modified", Some(&before))
        .await
        .unwrap();

      let err = undo(&conn, tx.id()).await.unwrap_err();
      let msg = err.to_string();
      assert!(msg.contains("unknown column"), "unexpected error: {msg}");
      assert!(msg.contains("not_a_real_column"), "unexpected error: {msg}");
    }

    #[tokio::test]
    async fn it_errors_when_table_name_is_not_in_the_allowlist() {
      let (_store, conn, _tmp, pid) = setup().await;

      // Record an event against a `table_name` outside the closed allowlist.
      // This could happen if the DB row was written by a future migration or
      // tampered with externally; either way, undo must refuse to interpolate
      // it into dynamic SQL.
      let tx = begin(&conn, &pid, "mystery op").await.unwrap();
      record_event(&conn, tx.id(), "not_a_real_table", "abc", "created", None)
        .await
        .unwrap();

      let err = undo(&conn, tx.id()).await.unwrap_err();
      let msg = err.to_string();
      assert!(msg.contains("unknown table"), "unexpected error: {msg}");
      assert!(msg.contains("not_a_real_table"), "unexpected error: {msg}");
    }

    #[tokio::test]
    async fn it_undoes_a_semantic_modified_event_via_before_data() {
      let (_store, conn, _tmp, pid) = setup().await;

      // Insert a task in the `open` state
      let task_id = Id::new();
      conn
        .execute(
          "INSERT INTO tasks (id, project_id, title, status) VALUES (?1, ?2, ?3, 'open')",
          [task_id.to_string(), pid.to_string(), "Task".to_string()],
        )
        .await
        .unwrap();

      // Capture before state, mutate, record semantic modified event
      let before = serde_json::json!({
        "id": task_id.to_string(),
        "status": "open",
        "title": "Task",
      });
      conn
        .execute(
          "UPDATE tasks SET status = 'in-progress' WHERE id = ?1",
          [task_id.to_string()],
        )
        .await
        .unwrap();

      let tx = begin(&conn, &pid, "task claim").await.unwrap();
      record_semantic_event(
        &conn,
        tx.id(),
        "tasks",
        &task_id.to_string(),
        "modified",
        Some(&before),
        Some("status-change"),
        Some("open"),
        Some("in-progress"),
      )
      .await
      .unwrap();

      // Undo should restore status to "open" from before_data, ignoring the semantic fields.
      undo(&conn, tx.id()).await.unwrap();

      let mut rows = conn
        .query("SELECT status FROM tasks WHERE id = ?1", [task_id.to_string()])
        .await
        .unwrap();
      let row = rows.next().await.unwrap().unwrap();
      let status: String = row.get(0).unwrap();

      assert_eq!(status, "open");
    }
  }
}
