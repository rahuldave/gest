//! `gest artifact meta set`: set a metadata value on an artifact at a dot-delimited path.

use clap::Args;

use crate::{AppContext, actions, cli::Error, ui::json};

/// Set a metadata value on an artifact at a dot-delimited path.
#[derive(Args, Debug)]
pub struct Command {
  /// The artifact ID or prefix.
  id: String,
  /// The dot-delimited metadata path.
  path: String,
  /// The metadata value (auto-detected scalar unless --as-json is set).
  value: String,
  /// Parse the value as a JSON literal instead of auto-detecting.
  #[arg(long)]
  as_json: bool,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Write the parsed value into the artifact's metadata at the given dot-path within a recorded transaction.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    actions::meta::set::<actions::Artifact>(context, &self.id, &self.path, &self.value, self.as_json, &self.output)
      .await
  }
}
