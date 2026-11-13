//! `gest config` subcommand tree for viewing and modifying settings.

mod get;
mod set;
mod show;

use clap::{Args, Subcommand};

use crate::{AppContext, cli::Error};

/// View or modify configuration.
#[derive(Args, Debug)]
pub struct Command {
  /// The subcommand to run.
  #[command(subcommand)]
  subcommand: Sub,
}

/// The subcommands of `gest config`, one variant for each.
#[derive(Debug, Subcommand)]
enum Sub {
  /// Get a configuration value.
  Get(get::Command),
  /// Set a configuration value.
  Set(set::Command),
  /// Show the current configuration.
  Show(show::Command),
}

impl Command {
  /// Dispatch to the matched config subcommand.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// The result of the subcommand that ran.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("config: entry");
    match &self.subcommand {
      Sub::Get(cmd) => cmd.call(context).await,
      Sub::Set(cmd) => cmd.call(context).await,
      Sub::Show(cmd) => cmd.call(context).await,
    }
  }
}
