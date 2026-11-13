//! Placeholder molecule shown in place of an empty list view.

use std::fmt::{self, Display, Formatter};

use yansi::Paint;

use crate::ui::style;

/// A styled placeholder message for empty entity lists.
pub struct Component {
  /// Plural entity name, shown as `no <entity>`.
  entity: &'static str,
}

impl Component {
  /// Create a new empty list placeholder.
  ///
  /// # Arguments
  ///
  /// * `entity` - the plural name of what is missing, such as `tasks`
  ///
  /// # Returns
  ///
  /// The placeholder, which displays as an indented, muted `no <entity>`.
  pub fn new(entity: &'static str) -> Self {
    Self {
      entity,
    }
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let theme = style::global();
    write!(f, "  {}", format!("no {}", self.entity).paint(*theme.muted()))
  }
}
