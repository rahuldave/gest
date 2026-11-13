//! `gest project detach`: detach the current directory from its project.

use clap::Args;

use crate::{AppContext, cli::Error, store::repo, ui::components::SuccessMessage};

/// Detach the current directory from its project.
#[derive(Args, Debug)]
pub struct Command;

impl Command {
  /// Remove the workspace row that points at the current working directory.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("project detach: entry");
    let cwd = std::env::current_dir()?;
    let conn = context.store().connect().await?;

    let detached = repo::project::detach_workspace(&conn, &cwd).await?;

    if !detached {
      return Err(Error::NotFound(format!("no workspace found for {}", cwd.display())));
    }

    let message = SuccessMessage::new("detached workspace").field("path", cwd.display().to_string());

    println!("{message}");
    Ok(())
  }
}
