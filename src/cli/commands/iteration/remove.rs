//! `gest iteration remove`: remove a task from an iteration.

use clap::Args;

use crate::{
  AppContext,
  cli::Error,
  store::{model::primitives::EntityType, repo},
  ui::{components::SuccessMessage, envelope::Envelope, json},
};

/// Remove a task from an iteration.
#[derive(Args, Debug)]
pub struct Command {
  /// The iteration ID or prefix.
  iteration: String,
  /// The task ID or prefix.
  task: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Detach the resolved task from the iteration by deleting its `iteration_tasks` row.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("iteration remove: entry");
    let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
    let conn = context.store().connect().await?;

    let iteration_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Iterations, &self.iteration).await?;
    let task_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Tasks, &self.task).await?;

    let tx = repo::transaction::begin(&conn, project_id, "iteration remove").await?;
    repo::iteration::remove_task_recorded(&conn, tx.id(), &iteration_id, &task_id).await?;

    let iteration = repo::iteration::find_required_by_id(&conn, iteration_id.clone()).await?;
    let short_id = iteration_id.short();
    let envelope = Envelope::load_one(&conn, EntityType::Iteration, &iteration_id, &iteration, true).await?;
    self.output.print_envelope(&envelope, &short_id, || {
      log::info!("removed task from iteration");
      SuccessMessage::new("removed task from iteration")
        .field("task", task_id.short())
        .field("iteration", iteration_id.short())
        .to_string()
    })?;
    Ok(())
  }
}
