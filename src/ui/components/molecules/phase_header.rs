//! Phase header molecule used by the iteration graph view.

use std::fmt::{self, Display, Formatter};

use yansi::Paint;

use crate::ui::components::atoms::Icon;

/// Two box-drawing dashes (`──`) that close the header line.
const SEPARATOR: &str = "\u{2500}\u{2500}";

/// Renders a phase header line: `◆  Phase N  ──`.
pub struct Component {
  /// Phase number shown as `Phase N`.
  number: u32,
}

impl Component {
  /// Create a phase header for the given phase number.
  ///
  /// # Arguments
  ///
  /// * `number` - the phase number
  ///
  /// # Returns
  ///
  /// The header, ready to display.
  pub fn new(number: u32) -> Self {
    Self {
      number,
    }
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let theme = crate::ui::style::global();
    let icon = Icon::phase();
    let label = format!("Phase {}", self.number);

    write!(
      f,
      "{}  {}  {}",
      icon,
      label.paint(*theme.iteration_graph_phase_label()),
      SEPARATOR.paint(*theme.iteration_graph_separator()),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn render(c: &Component) -> String {
    crate::ui::testing::plain(c)
  }

  #[test]
  fn it_renders_icon_label_and_separator() {
    let header = Component::new(2);

    let out = render(&header);

    assert!(out.contains('\u{25C6}'));
    assert!(out.contains("Phase 2"));
    assert!(out.contains("\u{2500}\u{2500}"));
  }
}
