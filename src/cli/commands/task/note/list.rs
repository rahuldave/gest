//! `gest task note list`: list notes on a task.

use clap::Args;

use crate::{
  AppContext, actions,
  cli::{Error, limit::LimitArgs},
  ui::json,
};

/// List notes on a task.
#[derive(Args, Debug)]
pub struct Command {
  /// The task ID or prefix.
  id: String,
  /// The `--limit` option, which caps how many items print.
  #[command(flatten)]
  limit: LimitArgs,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Render notes attached to the resolved task.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("task note list: entry");
    actions::note::list::<actions::Task>(context, &self.id, &self.limit, &self.output).await
  }
}
