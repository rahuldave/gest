//! `gest artifact note show`: show a single note.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Show a single note.
#[derive(Args, Debug)]
pub struct Command {
  /// The note ID or prefix.
  id: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Render the resolved note's body and metadata.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("artifact note show: entry");
    actions::note::show(context, &self.id, &self.output).await
  }
}
