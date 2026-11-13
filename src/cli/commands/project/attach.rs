//! `gest project attach`: attach the current directory to an existing project as a workspace.

use clap::Args;

use crate::{
  AppContext,
  cli::Error,
  store::{model::primitives::Id, repo},
  ui::components::SuccessMessage,
};

/// Attach the current directory to an existing project as a workspace.
#[derive(Args, Debug)]
pub struct Command {
  /// The project ID (or unique prefix) to attach to.
  id: String,
}

impl Command {
  /// Attach the current working directory to an existing project as a workspace.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("project attach: entry");
    let id: Id = self
      .id
      .parse()
      .map_err(|e: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let conn = context.store().connect().await?;
    let project = repo::project::find_by_id(&conn, id)
      .await?
      .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "project not found"))?;

    let cwd = std::env::current_dir()?;
    let ws = repo::project::attach_workspace(&conn, project.id(), &cwd).await?;

    let message = SuccessMessage::new("attached workspace")
      .id(ws.id().short())
      .field("project", project.id().short())
      .field("path", cwd.display().to_string());

    println!("{message}");
    Ok(())
  }
}
