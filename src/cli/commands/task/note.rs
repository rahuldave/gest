//! `task note` subcommand tree for managing notes attached to tasks.

mod add;
mod delete;
mod list;
mod show;
mod update;

use clap::{Args, Subcommand};

use crate::{AppContext, cli::Error};

/// Manage notes on a task.
#[derive(Args, Debug)]
pub struct Command {
  /// The subcommand to run.
  #[command(subcommand)]
  subcommand: Sub,
}

/// The subcommands of `gest task note`, one variant for each.
#[derive(Debug, Subcommand)]
enum Sub {
  /// Add a note to a task.
  Add(add::Command),
  /// Delete a note from a task.
  Delete(delete::Command),
  /// List notes on a task.
  List(list::Command),
  /// Show a single note.
  Show(show::Command),
  /// Update a note's body.
  Update(update::Command),
}

impl Command {
  /// Dispatch to the matched task note subcommand.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// The result of the subcommand that ran.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("task note: entry");
    match &self.subcommand {
      Sub::Add(cmd) => cmd.call(context).await,
      Sub::Delete(cmd) => cmd.call(context).await,
      Sub::List(cmd) => cmd.call(context).await,
      Sub::Show(cmd) => cmd.call(context).await,
      Sub::Update(cmd) => cmd.call(context).await,
    }
  }
}
