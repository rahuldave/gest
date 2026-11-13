//! `gest iteration untag`: remove a tag from an iteration.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Remove a tag from an iteration.
#[derive(Args, Debug)]
pub struct Command {
  /// The iteration ID or prefix.
  id: String,
  /// The tag label to remove.
  label: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Detach the given tag label from the resolved iteration within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    actions::tag::untag::<actions::Iteration>(context, &self.id, &self.label, &self.output).await
  }
}
