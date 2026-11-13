//! `gest task note delete`: delete a note from a task.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Delete a note from a task.
#[derive(Args, Debug)]
pub struct Command {
  /// The note ID or prefix.
  id: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
  /// Skip the interactive confirmation prompt.
  #[arg(long)]
  yes: bool,
}

impl Command {
  /// Confirm and delete the resolved note within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("task note delete: entry");
    actions::note::delete(context, &self.id, self.yes, "task", &self.output).await
  }
}
