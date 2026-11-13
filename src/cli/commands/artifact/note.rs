//! Subcommands for managing notes attached to artifacts.

mod add;
mod delete;
mod list;
mod show;
mod update;

use clap::{Args, Subcommand};

use crate::{AppContext, cli::Error};

/// Manage notes on an artifact.
#[derive(Args, Debug)]
pub struct Command {
  /// The subcommand to run.
  #[command(subcommand)]
  subcommand: Sub,
}

/// The subcommands of `gest artifact note`, one variant for each.
#[derive(Debug, Subcommand)]
enum Sub {
  /// Add a note to an artifact.
  Add(add::Command),
  /// Delete a note from an artifact.
  Delete(delete::Command),
  /// List notes on an artifact.
  List(list::Command),
  /// Show a single note.
  Show(show::Command),
  /// Update a note's body.
  Update(update::Command),
}

impl Command {
  /// Dispatch to the matched artifact note subcommand.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// The result of the subcommand that ran.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("artifact note: entry");
    match &self.subcommand {
      Sub::Add(cmd) => cmd.call(context).await,
      Sub::Delete(cmd) => cmd.call(context).await,
      Sub::List(cmd) => cmd.call(context).await,
      Sub::Show(cmd) => cmd.call(context).await,
      Sub::Update(cmd) => cmd.call(context).await,
    }
  }
}
