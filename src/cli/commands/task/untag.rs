//! `gest task untag`: remove a tag from a task.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Remove a tag from a task.
#[derive(Args, Debug)]
pub struct Command {
  /// The task ID or prefix.
  id: String,
  /// The tag label to remove.
  label: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Detach the given tag label from the resolved task within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    actions::tag::untag::<actions::Task>(context, &self.id, &self.label, &self.output).await
  }
}
