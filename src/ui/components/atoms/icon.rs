//! Unicode glyph indicator atom used for task status and blocking state.

use std::fmt::{self, Display, Formatter};

use yansi::{Paint, Style};

/// A unicode glyph indicator with a style.
pub struct Component {
  /// Single glyph to show.
  ch: char,
  /// Theme style painted over the glyph.
  style: Style,
}

impl Component {
  /// Blocked indicator icon.
  ///
  /// # Returns
  ///
  /// A `⊗` in the `indicator.blocked` style.
  pub fn blocked() -> Self {
    Self {
      ch: '\u{2297}',
      style: *crate::ui::style::global().indicator_blocked(),
    } // ⊗
  }

  /// Blocking indicator icon.
  ///
  /// # Returns
  ///
  /// A `!` in the `indicator.blocking` style.
  pub fn blocking() -> Self {
    Self {
      ch: '!',
      style: *crate::ui::style::global().indicator_blocking(),
    }
  }

  /// Phase header icon used by the iteration graph.
  ///
  /// # Returns
  ///
  /// A `◆` in the `iteration.graph.phase.icon` style.
  pub fn phase() -> Self {
    Self {
      ch: '\u{25C6}',
      style: *crate::ui::style::global().iteration_graph_phase_icon(),
    } // ◆
  }

  /// Status icon for the given status string.
  ///
  /// # Arguments
  ///
  /// * `status` - a task status: `open`, `in-progress`, `done` or `cancelled`
  ///
  /// # Returns
  ///
  /// `○`, `◐`, `●` or `⊘` in the matching `task.list.icon.*` style; an unknown status gets the open icon.
  pub fn status(status: &str) -> Self {
    let theme = crate::ui::style::global();
    match status {
      "open" => Self {
        ch: '\u{25CB}',
        style: *theme.task_list_icon_open(),
      }, // ○
      "in-progress" => Self {
        ch: '\u{25D0}',
        style: *theme.task_list_icon_in_progress(),
      }, // ◐
      "done" => Self {
        ch: '\u{25CF}',
        style: *theme.task_list_icon_done(),
      }, // ●
      "cancelled" => Self {
        ch: '\u{2298}',
        style: *theme.task_list_icon_cancelled(),
      }, // ⊘
      _ => Self {
        ch: '\u{25CB}',
        style: *theme.task_list_icon_open(),
      },
    }
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.ch.paint(self.style))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn render(icon: &Component) -> String {
    yansi::disable();
    let out = icon.to_string();
    yansi::enable();
    out
  }

  #[test]
  fn it_renders_blocked_icon() {
    assert_eq!(render(&Component::blocked()), "\u{2297}");
  }

  #[test]
  fn it_renders_blocking_icon() {
    assert_eq!(render(&Component::blocking()), "!");
  }

  #[test]
  fn it_renders_phase_icon() {
    assert_eq!(render(&Component::phase()), "\u{25C6}");
  }

  #[test]
  fn it_renders_status_icons() {
    assert_eq!(render(&Component::status("open")), "\u{25CB}");
    assert_eq!(render(&Component::status("in-progress")), "\u{25D0}");
    assert_eq!(render(&Component::status("done")), "\u{25CF}");
    assert_eq!(render(&Component::status("cancelled")), "\u{2298}");
  }
}
