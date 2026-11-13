//! `gest artifact meta get`: get a metadata value from an artifact by dot-delimited path.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Get a metadata value from an artifact by dot-delimited path.
#[derive(Args, Debug)]
pub struct Command {
  /// The artifact ID or prefix.
  id: String,
  /// The dot-delimited metadata path.
  path: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Resolve the artifact and print the metadata value at the given dot-path.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    actions::meta::get::<actions::Artifact>(context, &self.id, &self.path, &self.output).await
  }
}
