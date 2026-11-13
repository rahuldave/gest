//! Styled title atom with optional truncation and column padding.

use std::fmt::{self, Display, Formatter};

use yansi::{Paint, Style};

use crate::ui::components::molecules::row;

/// A styled heading with optional truncation and padding.
pub struct Component {
  /// Display width above which the text is cut to `max_width - 1` characters plus `…`; `None` never cuts.
  max_width: Option<usize>,
  /// Minimum visible width, filled with trailing spaces; `None` never pads.
  pad_to: Option<usize>,
  /// Style painted over the text.
  style: Style,
  /// Title text.
  text: String,
}

impl Component {
  /// Create a new title with the given text and style.
  ///
  /// # Arguments
  ///
  /// * `text` - the title text
  /// * `style` - the style to paint it in
  ///
  /// # Returns
  ///
  /// The title, with no truncation or padding.
  pub fn new(text: impl Into<String>, style: Style) -> Self {
    Self {
      max_width: None,
      pad_to: None,
      style,
      text: text.into(),
    }
  }

  /// Truncate the title to at most `width` visible characters, appending `…` if needed.
  ///
  /// # Arguments
  ///
  /// * `width` - the display width above which the title is cut to `width - 1` characters plus `…`;
  ///   wide characters can still exceed it
  ///
  /// # Returns
  ///
  /// The title with the limit set.
  pub fn max_width(mut self, width: usize) -> Self {
    self.max_width = Some(width);
    self
  }

  /// Right-pad with spaces so the title occupies at least `width` visible columns.
  ///
  /// # Arguments
  ///
  /// * `width` - the minimum visible width
  ///
  /// # Returns
  ///
  /// The title with the padding set.
  pub fn pad_to(mut self, width: usize) -> Self {
    self.pad_to = Some(width);
    self
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let visible = row::display_width(&self.text);
    let mut output = self.text.clone();

    if let Some(max) = self.max_width
      && visible > max
    {
      // Truncate to max-1 chars and append ellipsis
      let mut char_count = 0;
      let truncated: String = self
        .text
        .chars()
        .take_while(|_| {
          char_count += 1;
          char_count <= max.saturating_sub(1)
        })
        .collect();
      output = format!("{truncated}\u{2026}");
    }

    let styled = output.paint(self.style);
    write!(f, "{styled}")?;

    if let Some(pad) = self.pad_to {
      let current = row::display_width(&output);
      if current < pad {
        write!(f, "{}", " ".repeat(pad - current))?;
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn plain() -> Style {
    Style::default()
  }

  fn render(c: &Component) -> String {
    crate::ui::testing::plain(c)
  }

  #[test]
  fn it_pads_to_minimum_width() {
    let title = Component::new("hi", plain()).pad_to(10);

    let out = render(&title);

    assert_eq!(out.len(), 10);
  }

  #[test]
  fn it_truncates_then_pads() {
    let title = Component::new("very long title", plain()).max_width(6).pad_to(10);

    let out = render(&title);

    // 5 chars + ellipsis = 6 visible, padded to 10
    assert_eq!(row::display_width(&out), 10);
  }

  #[test]
  fn it_truncates_with_ellipsis() {
    let title = Component::new("long title text here", plain()).max_width(6);

    let out = render(&title);

    assert!(out.starts_with("long "));
    assert!(out.contains('\u{2026}'));
  }
}
