//! `gest task note update`: update a note's body.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Update a note's body.
#[derive(Args, Debug)]
pub struct Command {
  /// The note ID or prefix.
  id: String,
  /// The new body text (use `-` to open `$EDITOR`).
  #[arg(long, short)]
  body: Option<String>,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Replace the resolved note's body within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("task note update: entry");
    actions::note::update(context, &self.id, self.body.as_deref(), "task", &self.output).await
  }
}
