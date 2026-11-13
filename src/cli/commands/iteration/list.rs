//! `gest iteration list`: list iterations in the current project.

use clap::Args;

use crate::{
  AppContext,
  cli::{Error, limit::LimitArgs},
  store::{
    model::{
      iteration::Filter,
      primitives::{EntityType, Id, IterationStatus},
    },
    repo,
  },
  ui::{
    components::{IterationEntry, IterationListView},
    envelope::Envelope,
    json,
  },
};

/// List iterations in the current project.
#[derive(Args, Debug)]
pub struct Command {
  /// Show all iterations, including completed.
  #[arg(long, short)]
  all: bool,
  /// Only show iterations that have unclaimed (open) tasks.
  #[arg(long)]
  has_available: bool,
  /// The `--limit` option, which caps how many items print.
  #[command(flatten)]
  limit: LimitArgs,
  /// Filter by status.
  #[arg(long, short)]
  status: Option<IterationStatus>,
  /// Filter by tag.
  #[arg(long, short)]
  tag: Option<String>,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Query iterations with the requested filters and render them with per-iteration phase and task counts.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("iteration list: entry");
    let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
    let conn = context.store().connect().await?;

    let filter = Filter {
      all: self.all,
      has_available: self.has_available,
      status: self.status,
      tag: self.tag.clone(),
    };

    let mut iterations = repo::iteration::all(&conn, project_id, &filter).await?;
    self.limit.apply(&mut iterations);

    let id_shorts: Vec<String> = iterations.iter().map(|i| i.id().short().to_string()).collect();
    let full_ids: Vec<String> = iterations.iter().map(|i| i.id().to_string()).collect();

    if self.output.json {
      let pairs: Vec<(Id, &_)> = iterations.iter().map(|i| (i.id().clone(), i)).collect();
      let envelopes = Envelope::load_many(&conn, EntityType::Iteration, &pairs, false).await?;
      let json = serde_json::to_string_pretty(&envelopes)?;
      println!("{json}");
      return Ok(());
    }

    if self.output.quiet {
      self.output.print_short_ids(&id_shorts)?;
      return Ok(());
    }

    let full_id_refs: Vec<&str> = full_ids.iter().map(String::as_str).collect();
    let prefix_lengths = repo::iteration::prefix_lengths_for_project(&conn, project_id, &full_id_refs).await?;

    let iteration_ids: Vec<Id> = iterations.iter().map(|it| it.id().clone()).collect();
    let phase_map = repo::iteration::max_phases_batch(&conn, &iteration_ids).await?;
    let counts_map = repo::iteration::task_status_counts_batch(&conn, &iteration_ids).await?;

    let mut entries = Vec::new();
    for (i, (iteration, id_short)) in iterations.iter().zip(id_shorts.iter()).enumerate() {
      let phase_count = phase_map
        .get(iteration.id())
        .and_then(|m| *m)
        .map(|m| m as usize + 1)
        .unwrap_or(0);
      let status_counts = counts_map.get(iteration.id()).cloned().unwrap_or_default();
      let summary = format!(
        "{} {} · {} tasks",
        phase_count,
        if phase_count == 1 { "phase" } else { "phases" },
        status_counts.total,
      );
      entries.push(IterationEntry {
        id: id_short.clone(),
        prefix_len: prefix_lengths[i],
        summary,
        title: iteration.title().to_string(),
      });
    }

    crate::io::pager::page(&format!("{}\n", IterationListView::new(entries)), context)?;

    Ok(())
  }
}
