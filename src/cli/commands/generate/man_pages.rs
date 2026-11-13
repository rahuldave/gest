//! `gest generate man-pages`: generate man pages.

use std::path::PathBuf;

use clap::{Args, CommandFactory};
use clap_mangen::Man;

use crate::{
  AppContext,
  cli::{App, Error},
  ui::components::SuccessMessage,
};

/// Generate man pages.
#[derive(Args, Debug)]
pub struct Command {
  /// Output directory for man pages.
  #[arg(default_value = "man")]
  output_dir: PathBuf,
}

impl Command {
  /// Render the `gest.1` man page into the requested output directory.
  ///
  /// # Arguments
  ///
  /// * `_context` - the application context, which this command does not use
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, _context: &AppContext) -> Result<(), Error> {
    std::fs::create_dir_all(&self.output_dir)?;

    let cmd = App::command();
    let man = Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    let path = self.output_dir.join("gest.1");
    std::fs::write(&path, buffer)?;

    let message = SuccessMessage::new("generated man pages").field("output", path.display().to_string());
    println!("{message}");
    Ok(())
  }
}
