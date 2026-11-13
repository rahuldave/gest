//! Generic `meta` actions: show, get, set and unset an entity's JSON metadata.

use serde_json::{Map, Value};

use super::HasMetadata;
use crate::{
  AppContext,
  cli::Error,
  store::{meta, repo},
  ui::{
    components::{FieldList, MetaGet, SuccessMessage},
    envelope::Envelope,
    json,
  },
};

/// Display the full metadata blob for an entity.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_id` - the entity's ID or a unique prefix of it
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the metadata is printed, or the error from resolving or loading the entity.
pub async fn bare<E: HasMetadata>(context: &AppContext, raw_id: &str, output: &json::Flags) -> Result<(), Error> {
  let conn = context.store().connect().await?;
  let id = repo::resolve::resolve_id(&conn, E::table(), raw_id).await?;
  let model = E::find_by_id(&conn, id).await?;
  let metadata = E::metadata(&model);
  output.print_raw_or(metadata, || render_raw(metadata), || render_normal(metadata))
}

/// Resolve an entity and print a single metadata value at a dot-delimited path.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_id` - the entity's ID or a unique prefix of it
/// * `path` - the dot-delimited key path, such as `review.owner`
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the value is printed, [`Error::MetaKeyNotFound`] when `path` holds no value, or
/// the error from resolving or loading the entity.
pub async fn get<E: HasMetadata>(
  context: &AppContext,
  raw_id: &str,
  path: &str,
  output: &json::Flags,
) -> Result<(), Error> {
  let conn = context.store().connect().await?;
  let id = repo::resolve::resolve_id(&conn, E::table(), raw_id).await?;
  let model = E::find_by_id(&conn, id).await?;
  let value = meta::resolve_path(E::metadata(&model), path).ok_or_else(|| Error::MetaKeyNotFound(path.to_owned()))?;
  let mut wrapped = Map::new();
  wrapped.insert(path.to_owned(), value.clone());
  let wrapped = Value::Object(wrapped);
  output.print_raw_or(
    &wrapped,
    || meta::format_meta_value(value),
    || MetaGet::new(meta::format_meta_value(value)).to_string(),
  )
}

/// Set a metadata value on an entity at a dot-delimited path within a recorded transaction.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_id` - the entity's ID or a unique prefix of it
/// * `path` - the dot-delimited key path; missing or non-object values on the way become objects
/// * `value` - the value as typed on the command line
/// * `as_json` - parse `value` as JSON; otherwise it becomes a boolean, a number or a string
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the entity is updated and printed, [`Error::MetaKeyNotFound`] when `path` has
/// more segments than the store allows, a JSON error when `as_json` is set and `value` is not
/// JSON, or the error from resolving, loading or saving the entity.
pub async fn set<E: HasMetadata>(
  context: &AppContext,
  raw_id: &str,
  path: &str,
  value: &str,
  as_json: bool,
  output: &json::Flags,
) -> Result<(), Error> {
  let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
  let conn = context.store().connect().await?;
  let id = repo::resolve::resolve_id(&conn, E::table(), raw_id).await?;
  let model = E::find_by_id(&conn, id.clone()).await?;

  let parsed = if as_json {
    serde_json::from_str(value)?
  } else {
    meta::parse_scalar(value)
  };

  let mut metadata = E::metadata(&model).clone();
  if !metadata.is_object() {
    metadata = Value::Object(serde_json::Map::new());
  }
  if !meta::set_path(&mut metadata, path, parsed) {
    return Err(Error::MetaKeyNotFound(path.to_owned()));
  }

  let before = serde_json::to_value(&model)?;
  let event_table = E::event_table();
  let label = format!("{} meta set", event_table.trim_end_matches('s'));
  let tx = repo::transaction::begin(&conn, project_id, &label).await?;

  let patch = E::metadata_patch(metadata);
  E::update(&conn, &id, &patch).await?;
  repo::transaction::record_event(&conn, tx.id(), event_table, &id.to_string(), "modified", Some(&before)).await?;

  let updated = E::find_by_id(&conn, id.clone()).await?;
  let envelope = Envelope::load_one(&conn, E::entity_type(), &id, &updated, true).await?;
  let short_id = id.short();
  output.print_envelope(&envelope, &short_id, || {
    SuccessMessage::new("set metadata")
      .id(id.short())
      .field("path", path.to_owned())
      .to_string()
  })?;
  Ok(())
}

/// Remove a metadata value from an entity at a dot-delimited path within a recorded transaction.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_id` - the entity's ID or a unique prefix of it
/// * `path` - the dot-delimited key path to remove
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the entity is updated and printed, [`Error::MetaKeyNotFound`] when `path` holds
/// no value, or the error from resolving, loading or saving the entity.
pub async fn unset<E: HasMetadata>(
  context: &AppContext,
  raw_id: &str,
  path: &str,
  output: &json::Flags,
) -> Result<(), Error> {
  let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
  let conn = context.store().connect().await?;
  let id = repo::resolve::resolve_id(&conn, E::table(), raw_id).await?;
  let model = E::find_by_id(&conn, id.clone()).await?;

  let mut metadata = E::metadata(&model).clone();
  if !meta::unset_path(&mut metadata, path) {
    return Err(Error::MetaKeyNotFound(path.to_owned()));
  }

  let before = serde_json::to_value(&model)?;
  let event_table = E::event_table();
  let label = format!("{} meta unset", event_table.trim_end_matches('s'));
  let tx = repo::transaction::begin(&conn, project_id, &label).await?;

  let patch = E::metadata_patch(metadata);
  E::update(&conn, &id, &patch).await?;
  repo::transaction::record_event(&conn, tx.id(), event_table, &id.to_string(), "modified", Some(&before)).await?;

  let updated = E::find_by_id(&conn, id.clone()).await?;
  let envelope = Envelope::load_one(&conn, E::entity_type(), &id, &updated, true).await?;
  let short_id = id.short();
  output.print_envelope(&envelope, &short_id, || {
    SuccessMessage::new("unset metadata")
      .id(id.short())
      .field("path", path.to_owned())
      .to_string()
  })?;
  Ok(())
}

/// Render metadata as a themed field list, one dot-path for each leaf value.
///
/// # Arguments
///
/// * `metadata` - the metadata to render
///
/// # Returns
///
/// The field list, or `(no metadata)` when there is nothing to show.
fn render_normal(metadata: &Value) -> String {
  let pairs = meta::flatten_dot_paths(metadata);
  if pairs.is_empty() {
    return "(no metadata)".to_string();
  }
  let mut list = FieldList::new();
  for (path, value) in pairs {
    list = list.field(path, value);
  }
  list.to_string()
}

/// Render metadata as plain `path=value` lines for scripts.
///
/// # Arguments
///
/// * `metadata` - the metadata to render
///
/// # Returns
///
/// One `path=value` line for each leaf value, joined with newlines; empty when there is none.
fn render_raw(metadata: &Value) -> String {
  let pairs = meta::flatten_dot_paths(metadata);
  pairs
    .into_iter()
    .map(|(k, v)| format!("{k}={v}"))
    .collect::<Vec<_>>()
    .join("\n")
}
