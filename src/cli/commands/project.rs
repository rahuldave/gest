//! `gest project` subcommand tree for showing, archiving, attaching, and listing projects.

mod archive;
mod attach;
mod delete;
mod detach;
mod list;
mod unarchive;

use clap::{Args, Subcommand};

use crate::{AppContext, cli::Error, store::repo, ui::components::ProjectShow};

/// Show or manage the current project.
#[derive(Args, Debug)]
pub struct Command {
  /// Emit output as JSON (only applies to the default show view).
  #[arg(long)]
  json: bool,
  /// The subcommand to run; with none, the command shows the current project.
  #[command(subcommand)]
  subcommand: Option<Sub>,
}

/// The subcommands of `gest project`, one variant for each.
#[derive(Debug, Subcommand)]
enum Sub {
  /// Soft-archive a project, hiding it and its entities from default list views.
  Archive(archive::Command),
  /// Attach the current directory to an existing project as a workspace.
  Attach(attach::Command),
  /// Delete a project and all of its owned entities.
  #[command(visible_alias = "rm")]
  Delete(delete::Command),
  /// Detach the current directory from its project.
  Detach(detach::Command),
  /// List all known projects.
  #[command(visible_alias = "ls")]
  List(list::Command),
  /// Restore an archived project to active status.
  Unarchive(unarchive::Command),
}

impl Command {
  /// Dispatch to the matched project subcommand, or show the current project.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// The result of the subcommand, or of [`Self::show`] when there is none.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("project: entry");
    match &self.subcommand {
      Some(Sub::Archive(command)) => command.call(context).await,
      Some(Sub::Attach(command)) => command.call(context).await,
      Some(Sub::Delete(command)) => command.call(context).await,
      Some(Sub::Detach(command)) => command.call(context).await,
      Some(Sub::List(command)) => command.call(context).await,
      Some(Sub::Unarchive(command)) => command.call(context).await,
      None => self.show(context).await,
    }
  }

  /// Show the current project's ID and root, as a view or as JSON with `--json`.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` once the project is printed, [`Error::UninitializedProject`] outside a project,
  /// or [`Error::NotFound`] when the project's row is missing.
  async fn show(&self, context: &AppContext) -> Result<(), Error> {
    let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
    let conn = context.store().connect().await?;
    let project = repo::project::find_by_id(&conn, project_id.clone())
      .await?
      .ok_or_else(|| Error::NotFound(format!("project not found: {project_id}")))?;

    if self.json {
      let json = serde_json::to_string_pretty(&project)?;
      println!("{json}");
      return Ok(());
    }

    let view = ProjectShow::new(project.id().short(), project.root().display().to_string());
    println!("{view}");
    Ok(())
  }
}
