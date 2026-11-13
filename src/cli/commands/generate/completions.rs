//! `gest generate completions`: generate shell completions.

use std::io;

use clap::{Args, CommandFactory};
use clap_complete::{Shell, generate};

use crate::{
  AppContext,
  cli::{App, Error},
};

/// Generate shell completions.
#[derive(Args, Debug)]
pub struct Command {
  /// The shell to generate completions for.
  shell: Shell,
}

impl Command {
  /// Write shell completions for the requested shell to stdout.
  ///
  /// # Arguments
  ///
  /// * `_context` - the application context, which this command does not use
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, _context: &AppContext) -> Result<(), Error> {
    let mut cmd = App::command();
    generate(self.shell, &mut cmd, "gest", &mut io::stdout());
    Ok(())
  }
}
