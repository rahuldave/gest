//! `gest artifact note list`: list notes on an artifact.

use clap::Args;

use crate::{
  AppContext, actions,
  cli::{Error, limit::LimitArgs},
  ui::json,
};

/// List notes on an artifact.
#[derive(Args, Debug)]
pub struct Command {
  /// The artifact ID or prefix.
  id: String,
  /// The `--limit` option, which caps how many items print.
  #[command(flatten)]
  limit: LimitArgs,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Render notes attached to the resolved artifact.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("artifact note list: entry");
    actions::note::list::<actions::Artifact>(context, &self.id, &self.limit, &self.output).await
  }
}
