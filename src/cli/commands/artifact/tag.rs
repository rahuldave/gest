//! `gest artifact tag`: add a tag to an artifact.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Add a tag to an artifact.
#[derive(Args, Debug)]
pub struct Command {
  /// The artifact ID or prefix.
  id: String,
  /// The tag label to add.
  label: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Attach the given tag label to the resolved artifact within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    actions::tag::tag::<actions::Artifact>(context, &self.id, &self.label, &self.output).await
  }
}
