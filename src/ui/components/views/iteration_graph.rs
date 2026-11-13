//! Phased dependency graph with box-drawing connectors.
//!
//! Composes the [`PhaseHeader`] and [`TaskRow`] molecules with inline
//! branch-connector helpers to render an iteration as a phased task graph.

use std::{
  collections::BTreeMap,
  fmt::{self, Display, Formatter},
};

use yansi::Paint;

use crate::ui::components::molecules::{PhaseHeader, TaskRow, priority_pad_width};

/// `╰`: starts the closing connector of the last phase, where the graph ends.
const BRANCH_CLOSE_LAST: &str = "\u{2570}";
/// `├`: starts the closing connector of a phase that has another phase after it.
const BRANCH_CLOSE_MID: &str = "\u{251C}";
/// `─`: joins the tips of an opening or closing connector.
const BRANCH_DASH: &str = "\u{2500}";
/// `├`: starts the opening connector of a phase with more than one task.
const BRANCH_OPEN: &str = "\u{251C}";
/// `╮`: opens one more parallel column in an opening connector.
const BRANCH_OPEN_TIP: &str = "\u{256E}";
/// `╯`: closes one parallel column in a closing connector.
const CLOSE_TIP: &str = "\u{256F}";
/// `│`: the line between one phase and the next.
const CONTINUATION: &str = "\u{2502}";

/// Phased dependency graph with box-drawing connectors.
pub struct Component {
  /// Title on the graph's first line.
  iteration_title: String,
  /// Tasks in any order; the graph groups them by phase, lowest phase first.
  tasks: Vec<GraphTask>,
}

/// A task entry for the graph.
pub struct GraphTask {
  /// Short ids of tasks that block this task.
  pub blocked_by: Vec<String>,
  /// Short ID used to render the task's highlighted two-tone identifier.
  pub id_short: String,
  /// True when this task blocks at least one other task.
  pub is_blocking: bool,
  /// Phase number used to group tasks under phase headers.
  pub phase: u32,
  /// Number of highlighted prefix characters for this task's ID.
  pub prefix_len: usize,
  /// Task priority used to render the `[Pn]` badge.
  pub priority: Option<u8>,
  /// Task status, used to select the row icon and color.
  pub status: String,
  /// Task title rendered inside the row.
  pub title: String,
}

impl Component {
  /// Create a graph view for the given iteration title and task list.
  ///
  /// # Arguments
  ///
  /// * `iteration_title` - the iteration title
  /// * `tasks` - the tasks to draw; an empty list renders `no tasks in this iteration`
  ///
  /// # Returns
  ///
  /// The graph view.
  pub fn new(iteration_title: impl Into<String>, tasks: Vec<GraphTask>) -> Self {
    Self {
      iteration_title: iteration_title.into(),
      tasks,
    }
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let theme = crate::ui::style::global();

    writeln!(f)?;
    writeln!(f, "  {}", self.iteration_title.paint(*theme.iteration_graph_title()))?;

    if self.tasks.is_empty() {
      writeln!(f)?;
      writeln!(f, "  no tasks in this iteration")?;
      return Ok(());
    }

    let mut phases: BTreeMap<u32, Vec<&GraphTask>> = BTreeMap::new();
    for task in &self.tasks {
      phases.entry(task.phase).or_default().push(task);
    }

    let phase_count = phases.len();
    let task_count = self.tasks.len();
    let phase_word = if phase_count == 1 { "phase" } else { "phases" };
    let task_word = if task_count == 1 { "task" } else { "tasks" };
    writeln!(
      f,
      "  {}",
      format!("{phase_count} {phase_word} \u{00B7} {task_count} {task_word}").paint(*theme.list_summary())
    )?;
    writeln!(f)?;

    for (i, (phase, phase_tasks)) in phases.iter().enumerate() {
      let is_last_phase = i == phase_count - 1;
      let col_count = phase_tasks.len();

      writeln!(f, "  {}", PhaseHeader::new(*phase))?;

      if col_count > 1 {
        write!(f, "  ")?;
        write_branch_open(f, col_count)?;
        writeln!(f)?;
      }

      let priority_pad = priority_pad_width(phase_tasks.iter().map(|t| t.priority));

      for (ti, task) in phase_tasks.iter().enumerate() {
        let row = TaskRow::new(
          ti,
          col_count,
          &task.id_short,
          &task.title,
          &task.status,
          task.priority,
          priority_pad,
          task.prefix_len,
          &task.blocked_by,
          task.is_blocking,
        );
        writeln!(f, "  {row}")?;
      }

      if col_count > 1 {
        write!(f, "  ")?;
        write_branch_close(f, col_count, is_last_phase)?;
        writeln!(f)?;
      }

      if !is_last_phase {
        writeln!(f, "  {}", CONTINUATION.paint(*theme.iteration_graph_branch()))?;
      }
    }

    Ok(())
  }
}

/// Write the connector that merges a phase's parallel columns back into one line.
///
/// # Arguments
///
/// * `f` - the formatter to write into
/// * `cols` - the number of tasks in the phase; one `─╯` is written for each column after the first
/// * `is_last_phase` - whether the phase ends the graph, which starts the line with `╰` rather than `├`
///
/// # Returns
///
/// `Ok(())`, or the error from writing to `f`.
fn write_branch_close(f: &mut Formatter<'_>, cols: usize, is_last_phase: bool) -> fmt::Result {
  let theme = crate::ui::style::global();
  let branch = *theme.iteration_graph_branch();
  let start = if is_last_phase {
    BRANCH_CLOSE_LAST
  } else {
    BRANCH_CLOSE_MID
  };
  write!(f, "{}", start.paint(branch))?;
  for _ in 1..cols {
    write!(f, "{}{}", BRANCH_DASH.paint(branch), CLOSE_TIP.paint(branch))?;
  }
  Ok(())
}

/// Write the connector that splits a phase into parallel columns, one per task.
///
/// # Arguments
///
/// * `f` - the formatter to write into
/// * `cols` - the number of tasks in the phase; one `─╮` is written for each column after the first
///
/// # Returns
///
/// `Ok(())`, or the error from writing to `f`.
fn write_branch_open(f: &mut Formatter<'_>, cols: usize) -> fmt::Result {
  let theme = crate::ui::style::global();
  let branch = *theme.iteration_graph_branch();
  write!(f, "{}", BRANCH_OPEN.paint(branch))?;
  for _ in 1..cols {
    write!(f, "{}{}", BRANCH_DASH.paint(branch), BRANCH_OPEN_TIP.paint(branch))?;
  }
  Ok(())
}
