//! `gest task list`: list tasks in the current project.

use clap::Args;

use crate::{
  AppContext,
  cli::{Error, limit::LimitArgs},
  store::{
    model::{
      primitives::{EntityType, Id, TaskStatus},
      task::Filter,
    },
    repo,
  },
  ui::{
    components::{TaskEntry, TaskListView},
    envelope::Envelope,
    json,
  },
};

/// List tasks in the current project.
#[derive(Args, Debug)]
pub struct Command {
  /// Show all tasks, including resolved.
  #[arg(long, short)]
  all: bool,
  /// Filter by assigned author name.
  #[arg(long)]
  assigned_to: Option<String>,
  /// The `--limit` option, which caps how many items print.
  #[command(flatten)]
  limit: LimitArgs,
  /// Filter by status.
  #[arg(long, short)]
  status: Option<TaskStatus>,
  /// Filter by tag.
  #[arg(long, short)]
  tag: Option<String>,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Query tasks with the requested filters and render them as a table, JSON, or plain IDs.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("task list: entry");
    let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
    let conn = context.store().connect().await?;

    let filter = Filter {
      all: self.all,
      assigned_to: self.assigned_to.clone(),
      status: self.status,
      tag: self.tag.clone(),
    };

    let mut tasks = repo::task::all(&conn, project_id, &filter).await?;
    self.limit.apply(&mut tasks);

    if self.output.quiet {
      self.output.print_short_ids(tasks.iter().map(|t| t.id().short()))?;
      return Ok(());
    }

    if self.output.json {
      let pairs: Vec<(Id, &_)> = tasks.iter().map(|t| (t.id().clone(), t)).collect();
      let envelopes = Envelope::load_many(&conn, EntityType::Task, &pairs, false).await?;
      self.output.print_envelopes(
        &envelopes,
        || unreachable!("json flag is set"),
        || unreachable!("json flag is set"),
      )?;
      return Ok(());
    }

    let prefix_map = repo::task::per_id_prefix_lengths(&conn, project_id).await?;

    let task_ids: Vec<Id> = tasks.iter().map(|t| t.id().clone()).collect();
    let tag_map = repo::tag::for_entities(&conn, EntityType::Task, &task_ids).await?;

    let mut entries = Vec::new();
    for task in &tasks {
      let tags: Vec<String> = tag_map
        .get(task.id())
        .map(|tags| tags.iter().map(|t| t.label().to_string()).collect())
        .unwrap_or_default();
      let prefix_len = prefix_map.get(&task.id().to_string()).copied().unwrap_or(1);
      entries.push(TaskEntry {
        blocked_by: None,
        blocked_by_prefix_len: None,
        blocking: false,
        id: task.id().short().to_string(),
        prefix_len,
        priority: task.priority(),
        status: task.status().to_string(),
        tags,
        title: task.title().to_string(),
      });
    }

    crate::io::pager::page(&format!("{}\n", TaskListView::new(entries)), context)?;

    Ok(())
  }
}
