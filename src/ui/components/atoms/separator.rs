//! Horizontal rule atom with optional centered label.

use std::fmt::{self, Display, Formatter};

use yansi::{Paint, Style};

use crate::ui::components::molecules::row;

/// A horizontal separator line with optional centered label.
pub struct Component {
  /// Glyph repeated to draw the line: `─` for solid, `╌` for dashed.
  ch: char,
  /// Text centred in the line, with a space on each side.
  label: Option<String>,
  /// Style painted over the line and its label.
  style: Style,
  /// Total width in columns, with the label measured in bytes; `None` uses [`row::terminal_width`].
  width: Option<usize>,
}

impl Component {
  /// Create a solid rule separator.
  ///
  /// # Arguments
  ///
  /// * `style` - the style of the line
  ///
  /// # Returns
  ///
  /// A solid, unlabeled separator as wide as the terminal.
  pub fn rule(style: Style) -> Self {
    Self {
      ch: '─',
      label: None,
      style,
      width: None,
    }
  }

  /// Create a solid separator with a centered label.
  ///
  /// # Arguments
  ///
  /// * `label` - the text in the middle of the line
  /// * `style` - the style of the line and the label
  ///
  /// # Returns
  ///
  /// A solid, labeled separator as wide as the terminal.
  pub fn labeled(label: impl Into<String>, style: Style) -> Self {
    Self {
      ch: '─',
      label: Some(label.into()),
      style,
      width: None,
    }
  }

  /// Create a dashed separator with a centered label.
  ///
  /// # Arguments
  ///
  /// * `label` - the text in the middle of the line
  /// * `style` - the style of the line and the label
  ///
  /// # Returns
  ///
  /// A dashed, labeled separator as wide as the terminal.
  pub fn dashed(label: impl Into<String>, style: Style) -> Self {
    Self {
      ch: '╌',
      label: Some(label.into()),
      style,
      width: None,
    }
  }

  /// Override the separator width (defaults to the full terminal width).
  ///
  /// # Arguments
  ///
  /// * `width` - the total width in columns, label included
  ///
  /// # Returns
  ///
  /// The separator with the fixed width.
  pub fn width(mut self, width: usize) -> Self {
    self.width = Some(width);
    self
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let width = self.width.unwrap_or_else(|| row::terminal_width() as usize);
    let ch = self.ch.to_string();

    match &self.label {
      Some(label) => {
        let label_len = label.len() + 2; // " label "
        let left = (width.saturating_sub(label_len)) / 2;
        let right = width.saturating_sub(label_len + left);
        let left_line: String = ch.repeat(left);
        let right_line: String = ch.repeat(right);
        write!(
          f,
          "{} {} {}",
          left_line.paint(self.style),
          label.paint(self.style),
          right_line.paint(self.style),
        )
      }
      None => {
        let line: String = ch.repeat(width);
        write!(f, "{}", line.paint(self.style))
      }
    }
  }
}
