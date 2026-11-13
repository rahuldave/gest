//! Short styled text fragment atom, typically used for inline tags or status chips.

use std::fmt::{self, Display, Formatter};

use yansi::{Paint, Style};

/// A short styled text fragment.
pub struct Component {
  /// Style painted over the whole text.
  style: Style,
  /// Text shown as is, with no padding or truncation.
  text: String,
}

impl Component {
  /// Create a badge with the given text and style.
  ///
  /// # Arguments
  ///
  /// * `text` - the text to show
  /// * `style` - the style to paint it in
  ///
  /// # Returns
  ///
  /// The badge, ready to display.
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
