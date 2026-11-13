//! Styled text value for terminal output.

use std::fmt::{self, Display, Formatter};

use yansi::{Paint, Style};

/// A styled text value, typically paired with a [`Label`](super::Label) in detail views.
pub struct Component {
  /// Style painted over the whole text.
  style: Style,
  /// Text shown as is, with no padding or truncation.
  text: String,
}

impl Component {
  /// Create a value with the given display text and style.
  ///
  /// # Arguments
  ///
  /// * `text` - the text to show
  /// * `style` - the style to paint it in
  ///
  /// # Returns
  ///
  /// The value, ready to display.
  pub fn new(text: impl Into<String>, style: Style) -> Self {
    Self {
      style,
      text: text.into(),
    }
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.text.paint(self.style))
  }
}
