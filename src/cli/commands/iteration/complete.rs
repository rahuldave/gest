//! `gest iteration complete`: complete an iteration.

use clap::Args;

use crate::{
  AppContext,
  actions::{Iteration, transition::transition_status},
  cli::Error,
  store::model::primitives::IterationStatus,
  ui::json,
};

/// Complete an iteration.
#[derive(Args, Debug)]
pub struct Command {
  /// The iteration ID or prefix.
  id: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Transition the iteration to `completed` within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    transition_status::<Iteration>(
      context,
      &self.id,
      IterationStatus::Completed,
      "iteration complete",
      "completed",
      "completed iteration",
      &self.output,
    )
    .await
  }
}
