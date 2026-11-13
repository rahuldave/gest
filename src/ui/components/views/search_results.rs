//! Grouped search results view rendering task, artifact, and iteration sections.

use std::fmt::{self, Display, Formatter};

use yansi::Paint;

use super::{
  super::{atoms::Separator, molecules::EmptyList},
  search_result::Component as SearchResult,
};
use crate::{
  store::model::{artifact, iteration, task},
  ui::style,
};

/// Complete search results view with grouped sections and summary.
pub struct Component {
  /// Highlighted ID length for each artifact, index-aligned with `artifacts`.
  artifact_prefix_lens: Vec<usize>,
  /// Matching artifacts, listed in the `artifacts` section.
  artifacts: Vec<artifact::Model>,
  /// Whether each hit shows its description or body.
  expanded: bool,
  /// Highlighted ID length for each iteration, index-aligned with `iterations`.
  iteration_prefix_lens: Vec<usize>,
  /// Matching iterations, listed in the `iterations` section.
  iterations: Vec<iteration::Model>,
  /// Search text, echoed in the summary line.
  query: String,
  /// Highlighted ID length for each task, index-aligned with `tasks`.
  task_prefix_lens: Vec<usize>,
  /// Matching tasks, listed in the `tasks` section.
  tasks: Vec<task::Model>,
}

impl Component {
  /// Create a new search results view with per-ID prefix lengths.
  ///
  /// # Arguments
  ///
  /// * `query` - the search text, echoed in the summary
  /// * `tasks` - the matching tasks
  /// * `artifacts` - the matching artifacts
  /// * `iterations` - the matching iterations
  /// * `task_prefix_lens` - one prefix length per task; a task with no length is not
  ///   shown, but still counted in the totals
  /// * `artifact_prefix_lens` - one prefix length per artifact; an artifact with no length is not
  ///   shown, but still counted in the totals
  /// * `iteration_prefix_lens` - one prefix length per iteration; an iteration with no length is not
  ///   shown, but still counted in the totals
  ///
  /// # Returns
  ///
  /// A collapsed results view.
  pub fn new(
    query: String,
    tasks: Vec<task::Model>,
    artifacts: Vec<artifact::Model>,
    iterations: Vec<iteration::Model>,
    task_prefix_lens: Vec<usize>,
    artifact_prefix_lens: Vec<usize>,
    iteration_prefix_lens: Vec<usize>,
  ) -> Self {
    Self {
      artifact_prefix_lens,
      artifacts,
      expanded: false,
      iteration_prefix_lens,
      iterations,
      query,
      task_prefix_lens,
      tasks,
    }
  }

  /// Enable expanded mode to show full description/body per result.
  ///
  /// # Arguments
  ///
  /// * `expanded` - whether each hit shows its description or body
  ///
  /// # Returns
  ///
  /// The view with the mode set.
  pub fn expanded(mut self, expanded: bool) -> Self {
    self.expanded = expanded;
    self
  }

  /// Count the hits across the three sections.
  ///
  /// # Returns
  ///
  /// The number of tasks, artifacts and iterations together.
  fn total(&self) -> usize {
    self.tasks.len() + self.artifacts.len() + self.iterations.len()
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let theme = style::global();

    if self.total() == 0 {
      writeln!(f)?;
      write!(f, "{}", EmptyList::new("results"))?;
      return Ok(());
    }

    // Summary line
    writeln!(f)?;
    writeln!(
      f,
      "  found {} {} for '{}'",
      self.total().to_string().paint(*theme.search_summary()),
      if self.total() == 1 { "result" } else { "results" },
      self.query.paint(*theme.search_query()),
    )?;

    // Tasks section
    if !self.tasks.is_empty() {
      writeln!(f)?;
      writeln!(
        f,
        "  {}",
        Separator::labeled(format!("tasks ({})", self.tasks.len()), *theme.border())
      )?;
      for (task, &prefix_len) in self.tasks.iter().zip(self.task_prefix_lens.iter()) {
        writeln!(f)?;
        let body = if self.expanded {
          let d = task.description();
          if d.is_empty() { None } else { Some(d.to_string()) }
        } else {
          None
        };
        write!(
          f,
          "{}",
          SearchResult::task(
            task.id().short(),
            task.title().to_string(),
            task.status().to_string(),
            prefix_len,
          )
          .body(body)
          .expanded(self.expanded),
        )?;
      }
    }

    // Artifacts section
    if !self.artifacts.is_empty() {
      writeln!(f)?;
      writeln!(
        f,
        "  {}",
        Separator::labeled(format!("artifacts ({})", self.artifacts.len()), *theme.border()),
      )?;
      for (artifact, &prefix_len) in self.artifacts.iter().zip(self.artifact_prefix_lens.iter()) {
        writeln!(f)?;
        let body = if self.expanded {
          let b = artifact.body();
          if b.is_empty() { None } else { Some(b.to_string()) }
        } else {
          None
        };
        write!(
          f,
          "{}",
          SearchResult::artifact(artifact.id().short(), artifact.title().to_string(), prefix_len)
            .body(body)
            .expanded(self.expanded),
        )?;
      }
    }

    // Iterations section
    if !self.iterations.is_empty() {
      writeln!(f)?;
      writeln!(
        f,
        "  {}",
        Separator::labeled(format!("iterations ({})", self.iterations.len()), *theme.border()),
      )?;
      for (iteration, &prefix_len) in self.iterations.iter().zip(self.iteration_prefix_lens.iter()) {
        writeln!(f)?;
        let body = if self.expanded {
          let d = iteration.description();
          if d.is_empty() { None } else { Some(d.to_string()) }
        } else {
          None
        };
        write!(
          f,
          "{}",
          SearchResult::iteration(
            iteration.id().short(),
            iteration.title().to_string(),
            iteration.status().to_string(),
            prefix_len,
          )
          .body(body)
          .expanded(self.expanded),
        )?;
      }
    }

    Ok(())
  }
}
