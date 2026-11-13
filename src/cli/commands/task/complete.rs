//! `gest task complete`: mark a task as done.

use clap::Args;

use crate::{
  AppContext,
  actions::{Task, transition::transition_status},
  cli::Error,
  store::model::primitives::TaskStatus,
  ui::json,
};

/// Mark a task as done.
#[derive(Args, Debug)]
pub struct Command {
  /// The task ID or prefix.
  id: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Transition the resolved task to `done` within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    transition_status::<Task>(
      context,
      &self.id,
      TaskStatus::Done,
      "task complete",
      "completed",
      "completed task",
      &self.output,
    )
    .await
  }
}
