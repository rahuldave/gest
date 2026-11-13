//! `gest artifact` subcommand tree for creating, listing, and managing artifacts.

mod archive;
mod create;
mod delete;
mod list;
mod meta;
mod note;
mod show;
mod tag;
mod untag;
mod update;

use clap::{Args, Subcommand};

use crate::{AppContext, cli::Error};

/// Manage artifacts.
#[derive(Args, Debug)]
pub struct Command {
  /// The subcommand to run.
  #[command(subcommand)]
  subcommand: Sub,
}

/// The subcommands of `gest artifact`, one variant for each.
#[derive(Debug, Subcommand)]
enum Sub {
  /// Archive an artifact.
  Archive(archive::Command),
  /// Create a new artifact.
  #[command(visible_alias = "new")]
  Create(create::Command),
  /// Delete an artifact and its dependent rows.
  #[command(visible_alias = "rm")]
  Delete(delete::Command),
  /// List artifacts.
  #[command(visible_alias = "ls")]
  List(list::Command),
  /// Get or set custom metadata.
  Meta(meta::Command),
  /// Manage notes on an artifact.
  Note(note::Command),
  /// Show an artifact.
  #[command(visible_alias = "view")]
  Show(show::Command),
  /// Add a tag to an artifact.
  Tag(tag::Command),
  /// Remove a tag from an artifact.
  Untag(untag::Command),
  /// Update an artifact.
  #[command(visible_alias = "edit")]
  Update(update::Command),
}

impl Command {
  /// Dispatch to the matched artifact subcommand.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// The result of the subcommand that ran.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("artifact: entry");
    match &self.subcommand {
      Sub::Archive(cmd) => cmd.call(context).await,
      Sub::Create(cmd) => cmd.call(context).await,
      Sub::Delete(cmd) => cmd.call(context).await,
      Sub::List(cmd) => cmd.call(context).await,
      Sub::Meta(cmd) => cmd.call(context).await,
      Sub::Note(cmd) => cmd.call(context).await,
      Sub::Show(cmd) => cmd.call(context).await,
      Sub::Tag(cmd) => cmd.call(context).await,
      Sub::Untag(cmd) => cmd.call(context).await,
      Sub::Update(cmd) => cmd.call(context).await,
    }
  }
}
