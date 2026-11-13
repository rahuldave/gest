//! `gest iteration graph`: show phased task dependency graph.

use clap::Args;

use crate::{
  AppContext,
  cli::Error,
  store::repo,
  ui::{
    components::{GraphTask, IterationGraphView, unique_prefix_lengths},
    json,
  },
};

/// Show phased task dependency graph.
#[derive(Args, Debug)]
pub struct Command {
  /// The iteration ID or prefix.
  id: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Render the iteration's tasks grouped by phase as a dependency graph view.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("iteration graph: entry");
    let _project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
    let conn = context.store().connect().await?;
    let id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Iterations, &self.id).await?;
    let iteration = repo::iteration::find_required_by_id(&conn, id.clone()).await?;

    let tasks = repo::iteration::tasks_with_phase(&conn, &id).await?;

    if self.output.json {
      let json_tasks: Vec<_> = tasks
        .iter()
        .map(|t| {
          serde_json::json!({
            "blocked_by": t.blocked_by,
            "id": t.id_short,
            "is_blocking": t.is_blocking,
            "phase": t.phase,
            "priority": t.priority,
            "status": t.status,
            "title": t.title,
          })
        })
        .collect();
      let json = serde_json::json!({
        "id": id.to_string(),
        "title": iteration.title(),
        "tasks": json_tasks,
      });
      println!("{}", serde_json::to_string_pretty(&json)?);
      return Ok(());
    }

    if self.output.quiet {
      println!("{}", id.short());
      return Ok(());
    }

    let id_short_refs: Vec<&str> = tasks.iter().map(|t| t.id_short.as_str()).collect();
    let prefix_lengths = unique_prefix_lengths(&id_short_refs);

    let graph_tasks: Vec<GraphTask> = tasks
      .iter()
      .enumerate()
      .map(|(i, t)| GraphTask {
        blocked_by: t.blocked_by.clone(),
        id_short: t.id_short.clone(),
        is_blocking: t.is_blocking,
        phase: t.phase,
        prefix_len: prefix_lengths[i],
        priority: t.priority,
        status: t.status.clone(),
        title: t.title.clone(),
      })
      .collect();

    print!("{}", IterationGraphView::new(iteration.title(), graph_tasks));

    Ok(())
  }
}
