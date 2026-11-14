//! Generic `tag` and `untag` actions for any [`Taggable`] entity.

use super::Taggable;
use crate::{
  AppContext,
  cli::Error,
  store::repo,
  ui::{components::SuccessMessage, envelope::Envelope, json},
};

/// Attach a tag label to the entity identified by `raw_id`.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_id` - the entity's ID or a unique prefix of it
/// * `label` - the tag to attach
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the tag is attached and the entity printed, or the error that stopped it.
pub async fn tag<E: Taggable>(
  context: &AppContext,
  raw_id: &str,
  label: &str,
  output: &json::Flags,
) -> Result<(), Error> {
  let entity_name = E::entity_type().to_string();
  log::debug!("{entity_name} tag: entry");

  let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
  let conn = context.store().connect().await?;
  let id = repo::resolve::resolve_id(&conn, E::table(), raw_id).await?;

  let tx = repo::transaction::begin(&conn, project_id, &format!("{entity_name} tag")).await?;
  repo::tag::attach_recorded(&conn, tx.id(), E::entity_type(), &id, label).await?;

  let model = E::find_by_id(&conn, id.clone()).await?;
  let envelope = Envelope::load_one(&conn, E::entity_type(), &id, &model, true).await?;

  let prefix_len = E::prefix_length(&conn, project_id, &id.to_string()).await?;

  let short_id = id.short();
  output.print_envelope(&envelope, &short_id, || {
    let msg = format!("tagged {entity_name}");
    log::info!("{msg}");
    SuccessMessage::new(&msg)
      .id(id.short())
      .prefix_len(prefix_len)
      .field("tag", label.to_owned())
      .to_string()
  })?;
  Ok(())
}

/// Detach a tag label from the entity identified by `raw_id`.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_id` - the entity's ID or a unique prefix of it
/// * `label` - the tag to detach
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the tag is detached and the entity printed, or the error that stopped it.
pub async fn untag<E: Taggable>(
  context: &AppContext,
  raw_id: &str,
  label: &str,
  output: &json::Flags,
) -> Result<(), Error> {
  let entity_name = E::entity_type().to_string();
  log::debug!("{entity_name} untag: entry");

  let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
  let conn = context.store().connect().await?;
  let id = repo::resolve::resolve_id(&conn, E::table(), raw_id).await?;

  let tx = repo::transaction::begin(&conn, project_id, &format!("{entity_name} untag")).await?;
  let removed = repo::tag::detach_recorded(&conn, tx.id(), E::entity_type(), &id, label).await?;

  let model = E::find_by_id(&conn, id.clone()).await?;
  let envelope = Envelope::load_one(&conn, E::entity_type(), &id, &model, true).await?;

  let prefix_len = E::prefix_length(&conn, project_id, &id.to_string()).await?;

  let short_id = id.short();
  output.print_envelope(&envelope, &short_id, || {
    let msg = if removed {
      format!("untagged {entity_name}")
    } else {
      format!("{entity_name} was not tagged")
    };
    log::info!("{msg}");
    SuccessMessage::new(&msg)
      .id(short_id.clone())
      .prefix_len(prefix_len)
      .field("tag", label.to_owned())
      .to_string()
  })?;
  Ok(())
}
