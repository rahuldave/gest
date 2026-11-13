//! `gest generate` subcommand tree for emitting shell completions and man pages.

mod completions;
mod man_pages;

use clap::{Args, Subcommand};

use crate::{AppContext, cli::Error};

/// Generate shell completions and man pages.
#[derive(Args, Debug)]
pub struct Command {
  /// The subcommand to run.
  #[command(subcommand)]
  subcommand: Sub,
}

/// The subcommands of `gest generate`, one variant for each.
#[derive(Debug, Subcommand)]
enum Sub {
  /// Generate shell completions.
  Completions(completions::Command),
  /// Generate man pages.
  ManPages(man_pages::Command),
}

impl Command {
  /// Dispatch to the matched generate subcommand.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// The result of the subcommand that ran.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("generate: entry");
    match &self.subcommand {
      Sub::Completions(cmd) => cmd.call(context).await,
      Sub::ManPages(cmd) => cmd.call(context).await,
    }
  }
}
