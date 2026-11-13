//! Task row molecule used by the iteration graph view.
//!
//! Renders a single task line within a phase: parallel column indicators
//! (the task's status icon in its own column, `│` in sibling columns), the
//! task id, an optional `[Pn]` priority badge, a padded title, the status
//! badge, and any blocked/blocking indicators.

use std::fmt::{self, Display, Formatter};

use yansi::Paint;

use crate::ui::components::{
  atoms::{Badge, Icon, Id, Title},
  molecules::{Indicators, StatusBadge, row::display_width},
};

/// Max display width for task titles in iteration graph rows.
const TITLE_PAD: usize = 35;

/// Renders one task row inside a phase block of the iteration graph.
pub struct Component<'a> {
  /// IDs of the tasks that block this one; any blocker turns the icon and badge to blocked.
  blocked_by: &'a [String],
  /// Parallel column, counted from 0, that holds this task's icon.
  col_index: usize,
  /// Task ID to show; at most 8 characters are shown.
  id_short: &'a str,
  /// Whether the task blocks others, which adds the `! blocking` indicator.
  is_blocking: bool,
  /// Highlighted prefix length for this ID and the blocker IDs.
  prefix_len: usize,
  /// Priority shown as `[Pn]`; `None` leaves the badge slot blank.
  priority: Option<u8>,
  /// Width of the priority slot, from [`priority_pad_width`]; `0` omits the slot.
  priority_pad: usize,
  /// Task status, which picks the icon, badge and title style.
  status: &'a str,
  /// Task title, cut to 35 characters with `…` and padded to 35 columns.
  title: &'a str,
  /// Number of parallel columns in the phase; the others show `│`.
  total_cols: usize,
}

impl<'a> Component<'a> {
  /// Create a task row.
  ///
  /// # Arguments
  ///
  /// * `col_index` - the parallel column, counted from 0, that holds this task's icon
  /// * `total_cols` - the number of parallel columns in the phase
  /// * `id_short` - the task ID
  /// * `title` - the task title
  /// * `status` - the task status, such as `open` or `done`
  /// * `priority` - the task priority, shown as `[Pn]`
  /// * `priority_pad` - the width of the priority slot, from [`priority_pad_width`]; `0` omits it
  /// * `prefix_len` - the highlighted prefix length for this ID and the blocker IDs
  /// * `blocked_by` - the IDs of the tasks that block this one
  /// * `is_blocking` - whether the task blocks other tasks
  ///
  /// # Returns
  ///
  /// The row, ready to display.
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    col_index: usize,
    total_cols: usize,
    id_short: &'a str,
    title: &'a str,
    status: &'a str,
    priority: Option<u8>,
    priority_pad: usize,
    prefix_len: usize,
    blocked_by: &'a [String],
    is_blocking: bool,
  ) -> Self {
    Self {
      blocked_by,
      col_index,
      id_short,
      is_blocking,
      prefix_len,
      priority,
      priority_pad,
      status,
      title,
      total_cols,
    }
  }
}

impl Display for Component<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let theme = crate::ui::style::global();
    let is_blocked = !self.blocked_by.is_empty();

    for col in 0..self.total_cols {
      if col == self.col_index {
        let icon = if is_blocked {
          Icon::blocked()
        } else {
          Icon::status(self.status)
        };
        write!(f, "{icon}")?;
      } else {
        write!(f, "{}", "\u{2502}".paint(*theme.iteration_graph_branch()))?;
      }
      if col < self.total_cols - 1 {
        write!(f, " ")?;
      }
    }

    write!(f, "  {}", Id::new(self.id_short).prefix_len(self.prefix_len))?;

    if self.priority_pad > 0 {
      let badge = priority_badge(self.priority);
      let badge_str = badge.map(|b| b.to_string()).unwrap_or_default();
      let pad = self.priority_pad.saturating_sub(display_width(&badge_str));
      write!(f, "  {badge_str}{}", " ".repeat(pad))?;
    }

    let title_style = if self.status == "cancelled" {
      *theme.task_list_title_cancelled()
    } else {
      *theme.task_list_title()
    };
    let title = Title::new(self.title, title_style)
      .max_width(TITLE_PAD)
      .pad_to(TITLE_PAD);
    write!(f, "  {title}")?;

    let status_label = if is_blocked { "blocked" } else { self.status };
    write!(f, "  {}", StatusBadge::new(status_label))?;

    let blocked_by_refs: Vec<&str> = self.blocked_by.iter().map(String::as_str).collect();
    let indicators = Indicators::new()
      .blocking(self.is_blocking)
      .blocked_by(blocked_by_refs)
      .prefix_len(self.prefix_len);
    let indicators_str = indicators.to_string();
    if !indicators_str.is_empty() {
      write!(f, "  {indicators_str}")?;
    }

    Ok(())
  }
}

/// Compute the column width that fits the widest `[Pn]` priority badge in a set of tasks.
///
/// # Arguments
///
/// * `priorities` - the priority of each task; `None` has no badge
///
/// # Returns
///
/// The maximum rendered width of a priority badge for the given
/// task priorities, or `0` if none have one.
pub fn priority_pad_width(priorities: impl IntoIterator<Item = Option<u8>>) -> usize {
  priorities
    .into_iter()
    .map(|p| priority_badge(p).map(|b| display_width(&b.to_string())).unwrap_or(0))
    .max()
    .unwrap_or(0)
}

/// Build the `[Pn]` badge for a priority, in the `task.list.priority` style.
///
/// # Arguments
///
/// * `priority` - the task priority; `None` has no badge
///
/// # Returns
///
/// The badge, or `None` when the task has no priority.
fn priority_badge(priority: Option<u8>) -> Option<Badge> {
  let p = priority?;
  let theme = crate::ui::style::global();
  Some(Badge::new(format!("[P{p}]"), *theme.task_list_priority()))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn render(c: &Component) -> String {
    yansi::disable();
    let out = c.to_string();
    yansi::enable();
    out
  }

  #[test]
  fn it_renders_blocked_icon_when_task_has_blockers() {
    let blocked = vec!["aaaaaaaa".to_string()];
    let row = Component::new(0, 1, "bbbbbbbb", "title", "open", None, 0, 2, &blocked, false);

    let out = render(&row);

    assert!(out.contains('\u{2297}'));
  }

  #[test]
  fn it_renders_pipes_in_sibling_columns() {
    let row = Component::new(1, 3, "cccccccc", "title", "open", None, 0, 2, &[], false);

    let out = render(&row);

    assert!(out.starts_with("\u{2502} \u{25CB} \u{2502}"));
  }

  #[test]
  fn it_renders_priority_badge_when_present() {
    let row = Component::new(0, 1, "dddddddd", "title", "open", Some(1), 4, 2, &[], false);

    let out = render(&row);

    assert!(out.contains("[P1]"));
  }
}
