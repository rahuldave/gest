//! `gest search`: search across tasks, artifacts, and iterations.

use clap::Args;

use crate::{
  AppContext,
  cli::{Error, limit::LimitArgs},
  store::{repo, search_query},
  ui::components::SearchResults,
};

/// Search across tasks, artifacts, and iterations.
#[derive(Args, Debug)]
pub struct Command {
  /// The search term.
  query: String,
  /// Show full content without truncation.
  #[arg(short, long)]
  expand: bool,
  /// Emit results as JSON.
  #[arg(short, long)]
  json: bool,
  /// The `--limit` option, which caps how many items print.
  #[command(flatten)]
  limit: LimitArgs,
  /// Include resolved/archived entities.
  #[arg(short = 'a', long = "all")]
  show_all: bool,
}

impl Command {
  /// Run the search against the current project and render results.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("search: entry");
    let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
    let conn = context.store().connect().await?;

    let parsed = search_query::parse(&self.query);
    let mut results = repo::search::query(&conn, project_id, &parsed, self.show_all).await?;
    self.limit.apply(&mut results.tasks);
    self.limit.apply(&mut results.artifacts);
    self.limit.apply(&mut results.iterations);

    if self.json {
      let json_value = serde_json::json!({
        "query": self.query,
        "tasks": results.tasks,
        "artifacts": results.artifacts,
        "iterations": results.iterations,
      });
      let json = serde_json::to_string_pretty(&json_value)?;
      println!("{json}");
      return Ok(());
    }

    let task_prefix_map = repo::task::per_id_prefix_lengths(&conn, project_id).await?;
    let task_prefix_lens: Vec<usize> = results
      .tasks
      .iter()
      .map(|t| task_prefix_map.get(&t.id().to_string()).copied().unwrap_or(2))
      .collect();

    let artifact_ids: Vec<String> = results.artifacts.iter().map(|a| a.id().to_string()).collect();
    let artifact_refs: Vec<&str> = artifact_ids.iter().map(String::as_str).collect();
    let artifact_prefix_lens = repo::artifact::prefix_lengths(&conn, project_id, &artifact_refs).await?;

    let iteration_ids: Vec<String> = results.iterations.iter().map(|i| i.id().to_string()).collect();
    let iteration_refs: Vec<&str> = iteration_ids.iter().map(String::as_str).collect();
    let iteration_prefix_lens = repo::iteration::prefix_lengths_for_project(&conn, project_id, &iteration_refs).await?;

    let view = SearchResults::new(
      self.query.clone(),
      results.tasks,
      results.artifacts,
      results.iterations,
      task_prefix_lens,
      artifact_prefix_lens,
      iteration_prefix_lens,
    )
    .expanded(self.expand);
    crate::io::pager::page(&format!("{view}\n"), context)?;

    Ok(())
  }
}
