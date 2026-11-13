//! `gest task meta unset`: remove a metadata value from a task at a dot-delimited path.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Remove a metadata value from a task at a dot-delimited path.
#[derive(Args, Debug)]
pub struct Command {
  /// The task ID or prefix.
  id: String,
  /// The dot-delimited metadata path.
  path: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Remove the metadata value at the given dot-path from the task within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    actions::meta::unset::<actions::Task>(context, &self.id, &self.path, &self.output).await
  }
}
