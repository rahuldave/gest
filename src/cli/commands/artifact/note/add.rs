//! `gest artifact note add`: add a note to an artifact.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Add a note to an artifact.
#[derive(Args, Debug)]
pub struct Command {
  /// The artifact ID or prefix.
  id: String,
  /// The note body (use `-` to open `$EDITOR`).
  #[arg(short, long)]
  body: String,
  /// Set the author (agent) identifier for this note.
  #[arg(long)]
  agent: Option<String>,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Create a new note on the resolved artifact, resolving the author from flags or git identity.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("artifact note add: entry");
    actions::note::add::<actions::Artifact>(context, &self.id, &self.body, self.agent.as_deref(), &self.output).await
  }
}
