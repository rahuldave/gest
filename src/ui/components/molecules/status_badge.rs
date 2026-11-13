//! Status icon-plus-label molecule used in task rows and detail views.

use std::fmt::{self, Display, Formatter};

use yansi::{Paint, Style};

use crate::ui::components::atoms::Icon;

/// Renders a status icon + label for a given status string.
pub struct Component<'a> {
  /// Whether to show the label after the icon; only tests turn it off.
  show_text: bool,
  /// Status string: `open`, `in-progress`, `done`, `cancelled` or `blocked`.
  status: &'a str,
}

impl<'a> Component<'a> {
  /// Create a status badge for the given status string (e.g. `"open"`, `"in-progress"`).
  ///
  /// # Arguments
  ///
  /// * `status` - the status; an unknown one is shown with the open icon and style
  ///
  /// # Returns
  ///
  /// The badge, showing the icon and the label.
  pub fn new(status: &'a str) -> Self {
    Self {
      show_text: true,
      status,
    }
  }

  /// Hide the text label, showing only the icon.
  #[cfg(test)]
  pub fn icon_only(mut self) -> Self {
    self.show_text = false;
    self
  }

  /// Pick the icon: `⊗` for `blocked`, otherwise the [`Icon::status`] glyph.
  ///
  /// # Returns
  ///
  /// The icon for the status.
  fn icon(&self) -> Icon {
    match self.status {
      "blocked" => Icon::blocked(),
      _ => Icon::status(self.status),
    }
  }

  /// Pick the label text, which spells `in-progress` as `in progress`.
  ///
  /// # Returns
  ///
  /// The label, borrowed from the status or a static string.
  fn label(&self) -> &str {
    match self.status {
      "in-progress" => "in progress",
      other => other,
    }
  }

  /// Pick the label style from the theme's status tokens.
  ///
  /// # Returns
  ///
  /// The `status.*` style for the status; `indicator.blocked` for `blocked`; `status.open` for anything else.
  fn style(&self) -> Style {
    let theme = crate::ui::style::global();
    match self.status {
      "cancelled" => *theme.status_cancelled(),
      "done" => *theme.status_done(),
      "in-progress" => *theme.status_in_progress(),
      "blocked" => *theme.indicator_blocked(),
      _ => *theme.status_open(),
    }
  }
}

impl Display for Component<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.icon())?;
    if self.show_text {
      write!(f, " {}", self.label().paint(self.style()))?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn render(badge: &Component) -> String {
    crate::ui::testing::plain(badge)
  }

  #[test]
  fn it_renders_blocked_with_blocked_icon() {
    let badge = Component::new("blocked");
    let out = render(&badge);

    assert!(out.contains('\u{2297}'));
    assert!(out.contains("blocked"));
  }

  #[test]
  fn it_renders_icon_only() {
    let badge = Component::new("done").icon_only();
    let out = render(&badge);

    assert!(out.contains('\u{25CF}'));
    assert!(!out.contains("done"));
  }

  #[test]
  fn it_renders_in_progress_with_space() {
    let badge = Component::new("in-progress");
    let out = render(&badge);

    assert!(out.contains("in progress"));
  }

  #[test]
  fn it_renders_open_status() {
    let badge = Component::new("open");
    let out = render(&badge);

    assert!(out.contains('\u{25CB}'));
    assert!(out.contains("open"));
  }
}
