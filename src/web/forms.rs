//! Form-parsing helpers for the web handlers.

use libsql::Connection;
use serde::Deserialize;

use crate::store::{
  model::{
    primitives::{EntityType, Id, RelationshipType},
    relationship,
  },
  repo,
};

/// A pre-populated relationship link used by edit forms.
pub(crate) struct ExistingLink {
  /// The relationship as seen from the edited entity, such as `blocks`; inverted when the entity is
  /// the target.
  pub(crate) rel: String,
  /// The other entity as `<tasks|artifacts|iterations>/<id>`, the form's `link_ref[]` value.
  pub(crate) ref_: String,
}

/// Shared form body for note-add endpoints.
#[derive(Deserialize)]
pub(crate) struct NoteFormData {
  /// The note's Markdown text.
  pub(crate) body: String,
}

/// Build existing link items for edit form pre-population.
///
/// # Arguments
///
/// * `entity_id` - the entity being edited
/// * `entity_type` - the kind of the entity being edited
/// * `rels` - the relationships in which the entity is the source or the target
///
/// # Returns
///
/// One link for each relationship, in the order of `rels`.
pub(crate) fn build_existing_links_for_entity(
  entity_id: &Id,
  entity_type: EntityType,
  rels: &[relationship::Model],
) -> Vec<ExistingLink> {
  let mut links = Vec::new();
  for rel in rels {
    let (rel_label, other_id, other_type) = if rel.source_id() == entity_id && rel.source_type() == entity_type {
      (rel.rel_type().to_string(), rel.target_id().clone(), rel.target_type())
    } else {
      (
        rel.rel_type().inverse().to_string(),
        rel.source_id().clone(),
        rel.source_type(),
      )
    };

    let type_prefix = match other_type {
      EntityType::Task => "tasks",
      EntityType::Artifact => "artifacts",
      EntityType::Iteration => "iterations",
    };
    let ref_ = format!("{type_prefix}/{other_id}");
    links.push(ExistingLink {
      rel: rel_label,
      ref_,
    });
  }
  links
}

/// Extract repeated `link_rel[]` and `link_ref[]` fields from a url-encoded form body.
///
/// # Arguments
///
/// * `bytes` - the raw `application/x-www-form-urlencoded` body
///
/// # Returns
///
/// The `link_rel[]` values and the `link_ref[]` values, each in form order.
pub(crate) fn extract_link_fields(bytes: &[u8]) -> (Vec<String>, Vec<String>) {
  let mut rels = Vec::new();
  let mut refs = Vec::new();
  for (key, value) in form_urlencoded::parse(bytes) {
    match key.as_ref() {
      "link_rel[]" => rels.push(value.into_owned()),
      "link_ref[]" => refs.push(value.into_owned()),
      _ => {}
    }
  }
  (rels, refs)
}

/// Parse comma-separated tags string into trimmed, non-empty tag labels.
///
/// # Arguments
///
/// * `tags_str` - the tags field of a form, such as `"ui, bug"`
///
/// # Returns
///
/// The tag labels in input order; duplicates are kept.
pub(crate) fn parse_tags(tags_str: &str) -> Vec<String> {
  tags_str
    .split(',')
    .map(|t| t.trim().to_owned())
    .filter(|t| !t.is_empty())
    .collect()
}

/// Make the entity's outgoing links match the form's link rows, recording each change in `transaction_id`.
///
/// A link that is in the store and in the form is kept, one that is only in the store is deleted, and one that is
/// only in the form is created. A row with an unknown relationship, ref kind or ID is ignored. Each deletion and
/// creation is recorded, so `gest undo` restores the links as they were.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `transaction_id` - the transaction the changes belong to
/// * `entity_type` - the kind of the entity whose links these are
/// * `entity_id` - the entity
/// * `link_rels` - the relationship of each form row
/// * `link_refs` - the target of each form row, as `<kind>/<id>`
///
/// # Returns
///
/// `Ok(())` once the links match, or the store error as text.
pub(crate) async fn sync_form_links(
  conn: &Connection,
  transaction_id: &Id,
  entity_type: EntityType,
  entity_id: &Id,
  link_rels: &[String],
  link_refs: &[String],
) -> Result<(), String> {
  let mut wanted: Vec<(RelationshipType, EntityType, Id)> = Vec::new();
  for (rel_str, ref_str) in link_rels.iter().zip(link_refs.iter()) {
    let Ok(rel_type) = rel_str.parse::<RelationshipType>() else {
      continue;
    };
    let Some((kind, target_id_str)) = ref_str.split_once('/') else {
      continue;
    };
    let target_type = match kind {
      "tasks" => EntityType::Task,
      "artifacts" => EntityType::Artifact,
      "iterations" => EntityType::Iteration,
      _ => continue,
    };
    let Ok(target_id) = target_id_str.parse::<Id>() else {
      continue;
    };
    let row = (rel_type, target_type, target_id);
    if !wanted.contains(&row) {
      wanted.push(row);
    }
  }

  let existing = repo::relationship::for_entity(conn, entity_type, entity_id)
    .await
    .map_err(|e| e.to_string())?;
  let mut kept: Vec<(RelationshipType, EntityType, Id)> = Vec::new();
  for rel in existing
    .iter()
    .filter(|rel| rel.source_id() == entity_id && rel.source_type() == entity_type)
  {
    let row = (rel.rel_type(), rel.target_type(), rel.target_id().clone());
    if wanted.contains(&row) && !kept.contains(&row) {
      kept.push(row);
      continue;
    }
    let payload = repo::relationship::relationship_audit_payload(rel);
    repo::transaction::record_event(
      conn,
      transaction_id,
      "relationships",
      &rel.id().to_string(),
      "deleted",
      Some(&payload),
    )
    .await
    .map_err(|e| e.to_string())?;
    repo::relationship::delete(conn, rel.id())
      .await
      .map_err(|e| e.to_string())?;
  }

  for (rel_type, target_type, target_id) in wanted.into_iter().filter(|row| !kept.contains(row)) {
    let rel = repo::relationship::create(conn, rel_type, entity_type, entity_id, target_type, &target_id)
      .await
      .map_err(|e| e.to_string())?;
    repo::transaction::record_event(
      conn,
      transaction_id,
      "relationships",
      &rel.id().to_string(),
      "created",
      None,
    )
    .await
    .map_err(|e| e.to_string())?;
  }

  Ok(())
}

/// Make the entity's tags match the form's labels, recording each change in `transaction_id`.
///
/// A label that is stored and in the form is kept, one that is only stored is removed, and one that is only in the
/// form is added. A form that keeps every tag records nothing.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `transaction_id` - the transaction the changes belong to
/// * `entity_type` - the kind of the tagged entity
/// * `entity_id` - the entity
/// * `labels` - the form's tag labels
///
/// # Returns
///
/// `Ok(())` once the tags match, or the store error as text.
pub(crate) async fn sync_form_tags(
  conn: &Connection,
  transaction_id: &Id,
  entity_type: EntityType,
  entity_id: &Id,
  labels: &[String],
) -> Result<(), String> {
  let stored = repo::tag::for_entity(conn, entity_type, entity_id)
    .await
    .map_err(|e| e.to_string())?;
  for label in stored.iter().filter(|label| !labels.contains(label)) {
    repo::tag::detach_recorded(conn, transaction_id, entity_type, entity_id, label)
      .await
      .map_err(|e| e.to_string())?;
  }
  for label in labels.iter().filter(|label| !stored.contains(label)) {
    repo::tag::attach_recorded(conn, transaction_id, entity_type, entity_id, label)
      .await
      .map_err(|e| e.to_string())?;
  }
  Ok(())
}

/// Open a transaction for a write from the web UI, under the server's author when it has one.
///
/// # Arguments
///
/// * `state` - the shared server state, for the project and the author
/// * `conn` - the database connection
/// * `command` - the command name that `gest undo` shows, such as `task update`
///
/// # Returns
///
/// The new transaction, or the store error.
pub(crate) async fn begin(
  state: &crate::web::AppState,
  conn: &Connection,
  command: &str,
) -> Result<crate::store::model::transaction::Model, crate::store::Error> {
  repo::transaction::begin_with_author(conn, state.project_id(), command, state.author_id().as_ref()).await
}
