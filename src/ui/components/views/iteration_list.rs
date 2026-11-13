//! Full iteration list view with Grid-aligned columns and count summary.

use std::fmt::{self, Display, Formatter};

use crate::ui::components::{
  atoms::{Column, Id, Title},
  molecules::{EmptyList, Grid, GroupedList, Row},
};

/// A single iteration entry for the list view.
pub struct IterationEntry {
  /// Short ID string displayed as the leading column.
  pub id: String,
  /// Number of highlighted prefix characters for this entry's ID.
  pub prefix_len: usize,
  /// Pre-rendered summary fragment (e.g. task count breakdown) shown after the title.
  pub summary: String,
  /// Iteration title rendered in the title column.
  pub title: String,
}

/// Full iteration list view using Grid for column alignment.
pub struct Component {
  /// Iterations in display order; an empty list renders the empty-list message.
  entries: Vec<IterationEntry>,
}

impl Component {
  /// Create a list view from the entries, each carrying its own prefix length.
  ///
  /// # Arguments
  ///
  /// * `entries` - the iterations to list, in display order
  ///
  /// # Returns
  ///
  /// The list view.
  pub fn new(entries: Vec<IterationEntry>) -> Self {
    Self {
      entries,
    }
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    if self.entries.is_empty() {
      return write!(f, "{}", EmptyList::new("iterations"));
    }

    let theme = crate::ui::style::global();
    let count = self.entries.len();
    let summary_text = format!("{count} {}", if count == 1 { "iteration" } else { "iterations" });
    let mut grid = Grid::new().spacing(2);

    for entry in &self.entries {
      let id = Id::new(&entry.id).prefix_len(entry.prefix_len);
      let title = Title::new(&entry.title, *theme.iteration_list_title());

      let row = Row::new()
        .col(Column::natural(id))
        .col(Column::natural(title))
        .col(Column::natural(&entry.summary));

      grid.push(row);
    }

    let list = GroupedList::new("iterations", summary_text).row(grid.to_string());
    write!(f, "{list}")
  }
}
