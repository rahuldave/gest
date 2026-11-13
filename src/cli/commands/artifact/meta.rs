//! Subcommands for reading and writing artifact metadata.

mod get;
mod set;
mod unset;

use clap::{Args, Subcommand};

use crate::{AppContext, actions, cli::Error, ui::json};

/// Read or write artifact metadata fields.
#[derive(Args, Debug)]
pub struct Command {
  /// The artifact ID or prefix (used when no subcommand is given).
  id: Option<String>,
  /// The subcommand to run; with none, the command shows all the metadata of `id`.
  #[command(subcommand)]
  subcommand: Option<Sub>,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

/// The subcommands of `gest artifact meta`, one variant for each.
#[derive(Debug, Subcommand)]
enum Sub {
  /// Get a metadata value by dot-delimited path.
  Get(get::Command),
  /// Set a metadata value at a dot-delimited path.
  Set(set::Command),
  /// Remove a metadata value at a dot-delimited path.
  #[command(alias = "delete")]
  Unset(unset::Command),
}

impl Command {
  /// Dispatch to the matched artifact meta subcommand, or show all the metadata of `id`.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// The result of the subcommand; with none, the result of showing the metadata, or
  /// [`Error::Argument`] when `id` is missing.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("artifact meta: entry");
    match &self.subcommand {
      Some(Sub::Get(cmd)) => cmd.call(context).await,
      Some(Sub::Set(cmd)) => cmd.call(context).await,
      Some(Sub::Unset(cmd)) => cmd.call(context).await,
      None => {
        let id = self
          .id
          .as_deref()
          .ok_or_else(|| Error::Argument("<id> argument is required".to_string()))?;
        actions::meta::bare::<actions::Artifact>(context, id, &self.output).await
      }
    }
  }
}
