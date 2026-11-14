//! `gest tag remove`: remove tags from any entity (task, artifact, or iteration).

use clap::Args;

use crate::{
  AppContext,
  cli::{Error, tag_arg},
  store::repo,
  ui::{components::SuccessMessage, json},
};

/// Remove tags from any entity (task, artifact, or iteration).
#[derive(Args, Debug)]
pub struct Command {
  /// The entity ID or prefix.
  id: String,
  /// One or more tag labels to remove (comma-separated values split into multiple tags).
  #[arg(required = true, value_delimiter = ',', value_parser = tag_arg::trim_tag)]
  tags: Vec<String>,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Detach each requested tag from the resolved entity inside a single transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("tag remove: entry");
    let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
    let conn = context.store().connect().await?;
    let (entity_type, id) = repo::resolve::resolve_entity(&conn, &self.id).await?;

    let labels = tag_arg::normalize_tags(&self.tags);
    let tx = repo::transaction::begin(&conn, project_id, &format!("{entity_type} untag")).await?;
    for label in &labels {
      repo::tag::detach_recorded(&conn, tx.id(), entity_type, &id, label).await?;
    }

    let short_id = id.short();
    self.output.print_delete(|| {
      SuccessMessage::new(format!("untagged {entity_type}"))
        .id(short_id.clone())
        .field("tags", labels.join(", "))
        .to_string()
    })?;
    Ok(())
  }
}
