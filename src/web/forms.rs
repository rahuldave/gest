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

/// Parse parallel `link_rel[]` and `link_ref[]` form fields and sync the entity's relationships.
///
/// Every relationship whose source is the entity is deleted, then one is created for each pair of
/// fields. A pair with an unknown relationship type, a bad reference or an unparseable id is skipped.
/// Relationships in which the entity is only the target are left alone.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `entity_type` - the kind of the entity being saved
/// * `entity_id` - the entity being saved, the source of the new relationships
/// * `link_rels` - the relationship types, such as `blocks`
/// * `link_refs` - the targets, as `<tasks|artifacts|iterations>/<id>`, paired with `link_rels` by position
///
/// # Returns
///
/// `Ok(())`, or the database error as text.
pub(crate) async fn sync_form_links(
  conn: &Connection,
  entity_type: EntityType,
  entity_id: &Id,
  link_rels: &[String],
  link_refs: &[String],
) -> Result<(), String> {
  // Delete all existing relationships where this entity is the source
  let existing = repo::relationship::for_entity(conn, entity_type, entity_id)
    .await
    .map_err(|e| e.to_string())?;
  for rel in &existing {
    if rel.source_id() == entity_id && rel.source_type() == entity_type {
      repo::relationship::delete(conn, rel.id())
        .await
        .map_err(|e| e.to_string())?;
    }
  }

  // Create new relationships from form data
  for (rel_str, ref_str) in link_rels.iter().zip(link_refs.iter()) {
    let rel_type: RelationshipType = match rel_str.parse() {
      Ok(r) => r,
      Err(_) => continue,
    };

    // ref format: "tasks/{id}" or "artifacts/{id}"
    let parts: Vec<&str> = ref_str.splitn(2, '/').collect();
    if parts.len() != 2 {
      continue;
    }
    let (target_type, target_id_str) = match parts[0] {
      "tasks" => (EntityType::Task, parts[1]),
      "artifacts" => (EntityType::Artifact, parts[1]),
      "iterations" => (EntityType::Iteration, parts[1]),
      _ => continue,
    };
    let target_id: Id = match target_id_str.parse() {
      Ok(id) => id,
      Err(_) => continue,
    };

    repo::relationship::create(conn, rel_type, entity_type, entity_id, target_type, &target_id)
      .await
      .map_err(|e| e.to_string())?;
  }

  Ok(())
}
