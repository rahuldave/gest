//! `gest iteration meta unset`: remove a metadata value from an iteration at a dot-delimited path.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Remove a metadata value from an iteration at a dot-delimited path.
#[derive(Args, Debug)]
pub struct Command {
  /// The iteration ID or prefix.
  id: String,
  /// The dot-delimited metadata path.
  path: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Remove the metadata value at the given dot-path from the iteration within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    actions::meta::unset::<actions::Iteration>(context, &self.id, &self.path, &self.output).await
  }
}
