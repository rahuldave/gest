//! Layout atom declaring horizontal space intent within a Row.

use std::fmt::{self, Display, Formatter};

/// A single cell within a Row that declares how much horizontal space it wants.
///
/// Column is a pure layout atom — it does NOT measure or resolve widths. That is
/// the Row molecule's responsibility. Column only stores the content and the
/// flex intent.
pub struct Component {
  /// Rendered cell text, which may hold ANSI styling.
  content: String,
  /// How much horizontal space the cell asks the row for.
  flex: Flex,
}

/// How a column requests horizontal space.
#[derive(Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Flex {
  /// Requests an exact character width. Narrower content is padded; wider content is not cut.
  Fixed(usize),
  /// Requests a proportional share of remaining space after fixed and natural
  /// columns are allocated.
  Flex(u32),
  /// Requests exactly enough space to fit its content.
  Natural,
}

impl Component {
  /// Create a column that requests an exact character width.
  ///
  /// # Arguments
  ///
  /// * `width` - the width in columns; the row pads narrower content to it and never cuts wider content
  /// * `content` - the cell content, rendered to text now
  ///
  /// # Returns
  ///
  /// The fixed-width column.
  pub fn fixed(width: usize, content: impl Display) -> Self {
    Self {
      content: content.to_string(),
      flex: Flex::Fixed(width),
    }
  }

  /// Create a column that requests a proportional share of remaining space.
  ///
  /// # Arguments
  ///
  /// * `weight` - the column's share relative to the other flex columns in the row
  /// * `content` - the cell content, rendered to text now
  ///
  /// # Returns
  ///
  /// The flex column.
  pub fn flex(weight: u32, content: impl Display) -> Self {
    Self {
      content: content.to_string(),
      flex: Flex::Flex(weight),
    }
  }

  /// Create a column that requests exactly enough space for its content.
  ///
  /// # Arguments
  ///
  /// * `content` - the cell content, rendered to text now
  ///
  /// # Returns
  ///
  /// The natural-width column.
  pub fn natural(content: impl Display) -> Self {
    Self {
      content: content.to_string(),
      flex: Flex::Natural,
    }
  }

  /// Returns the content string.
  ///
  /// # Returns
  ///
  /// The content string.
  pub fn content(&self) -> &str {
    &self.content
  }

  /// Returns the flex intent for this column.
  ///
  /// # Returns
  ///
  /// The flex intent for this column.
  pub fn flex_intent(&self) -> Flex {
    self.flex
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.content)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_creates_fixed_column() {
    let col = Component::fixed(10, "hello");

    assert_eq!(col.content(), "hello");
    assert!(matches!(col.flex_intent(), Flex::Fixed(10)));
  }

  #[test]
  fn it_creates_flex_column() {
    let col = Component::flex(2, "content");

    assert_eq!(col.content(), "content");
    assert!(matches!(col.flex_intent(), Flex::Flex(2)));
  }

  #[test]
  fn it_creates_natural_column() {
    let col = Component::natural("test");

    assert_eq!(col.content(), "test");
    assert!(matches!(col.flex_intent(), Flex::Natural));
  }

  #[test]
  fn it_displays_content() {
    let col = Component::natural("hello world");

    assert_eq!(col.to_string(), "hello world");
  }
}
