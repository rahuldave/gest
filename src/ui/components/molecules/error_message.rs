//! Styled error message component for terminal output.

use std::fmt::{self, Display, Formatter};

use yansi::Paint;

/// A themed error message that renders as `ERROR <message>` with the label
/// styled according to the active [`Theme`](crate::ui::style::Theme).
pub struct Component {
  /// Message text shown after the `ERROR` label, unstyled.
  message: String,
}

impl Component {
  /// Create a new error message from any string-like value.
  ///
  /// # Arguments
  ///
  /// * `message` - the error text
  ///
  /// # Returns
  ///
  /// The message; the theme is read when it is displayed.
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{} {}",
      "ERROR".paint(*crate::ui::style::global().error()),
      self.message
    )
  }
}
