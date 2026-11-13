//! Single-hit search result view rendering entity type, ID, title, and optional body.

use std::fmt::{self, Display, Formatter};

use yansi::{Paint, Style};

use super::super::atoms::{Badge, Id, Separator};
use crate::ui::style;

/// A single search hit showing entity type, ID, title, and status/kind.
pub struct Component {
  /// Description or body text, shown in full under the hit only when `expanded` is set.
  body: Option<String>,
  /// Kind of entity, which sets the leading label.
  entity_type: EntityType,
  /// Whether to add a dashed header line and the body.
  expanded: bool,
  /// Short ID of the entity, shown with its prefix highlighted.
  id: String,
  /// Number of highlighted characters in the ID.
  prefix_len: usize,
  /// Status shown as a colored badge after the title; `None` for artifacts.
  status_or_kind: Option<String>,
  /// Entity title, shown unstyled.
  title: String,
}

/// Kind of entity a hit is, which sets its leading label and that label's color.
enum EntityType {
  /// An artifact, labeled `artifact` in the `status.open` style.
  Artifact,
  /// An iteration, labeled `iteration` in the `status.done` style.
  Iteration,
  /// A task, labeled `task` in the `status.in_progress` style.
  Task,
}

impl EntityType {
  /// The lowercase word shown for this kind of entity.
  ///
  /// # Returns
  ///
  /// `artifact`, `iteration` or `task`.
  fn label(&self) -> &'static str {
    match self {
      Self::Artifact => "artifact",
      Self::Iteration => "iteration",
      Self::Task => "task",
    }
  }
}

impl Component {
  /// Create a search result for an artifact.
  ///
  /// # Arguments
  ///
  /// * `id` - the artifact's short ID
  /// * `title` - the artifact title
  /// * `prefix_len` - the number of highlighted ID characters
  ///
  /// # Returns
  ///
  /// A collapsed hit with no status and no body.
  pub fn artifact(id: String, title: String, prefix_len: usize) -> Self {
    Self {
      body: None,
      entity_type: EntityType::Artifact,
      expanded: false,
      id,
      prefix_len,
      status_or_kind: None,
      title,
    }
  }

  /// Create a search result for an iteration.
  ///
  /// # Arguments
  ///
  /// * `id` - the iteration's short ID
  /// * `title` - the iteration title
  /// * `status` - the iteration status, shown as a badge
  /// * `prefix_len` - the number of highlighted ID characters
  ///
  /// # Returns
  ///
  /// A collapsed hit with no body.
  pub fn iteration(id: String, title: String, status: String, prefix_len: usize) -> Self {
    Self {
      body: None,
      entity_type: EntityType::Iteration,
      expanded: false,
      id,
      prefix_len,
      status_or_kind: Some(status),
      title,
    }
  }

  /// Create a search result for a task.
  ///
  /// # Arguments
  ///
  /// * `id` - the task's short ID
  /// * `title` - the task title
  /// * `status` - the task status, shown as a badge
  /// * `prefix_len` - the number of highlighted ID characters
  ///
  /// # Returns
  ///
  /// A collapsed hit with no body.
  pub fn task(id: String, title: String, status: String, prefix_len: usize) -> Self {
    Self {
      body: None,
      entity_type: EntityType::Task,
      expanded: false,
      id,
      prefix_len,
      status_or_kind: Some(status),
      title,
    }
  }

  /// Set the body/description content shown in expanded mode.
  ///
  /// # Arguments
  ///
  /// * `body` - the text to show, or `None` for none
  ///
  /// # Returns
  ///
  /// The hit with the body replaced.
  pub fn body(mut self, body: Option<String>) -> Self {
    self.body = body;
    self
  }

  /// Enable expanded rendering with full description and section separator.
  ///
  /// # Arguments
  ///
  /// * `expanded` - whether to show the dashed header and the body
  ///
  /// # Returns
  ///
  /// The hit with the mode set.
  pub fn expanded(mut self, expanded: bool) -> Self {
    self.expanded = expanded;
    self
  }
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let theme = style::global();

    if self.expanded {
      // Dashed separator header: "╌╌╌ entity_type id ╌╌╌"
      let label = format!("{} {}", self.entity_type.label(), self.id);
      write!(f, "  {}", Separator::dashed(label, *theme.search_expand_separator()),)?;
      writeln!(f)?;
    }

    // Entity type badge with color based on type
    let type_style = match self.entity_type {
      EntityType::Task => theme.status_in_progress(),
      EntityType::Artifact => theme.status_open(),
      EntityType::Iteration => theme.status_done(),
    };
    write!(f, "  {}", self.entity_type.label().paint(*type_style))?;

    // ID
    write!(f, "  {}", Id::new(&self.id).prefix_len(self.prefix_len))?;

    // Title
    write!(f, "  {}", self.title)?;

    // Status badge or kind label
    if let Some(ref value) = self.status_or_kind {
      match self.entity_type {
        EntityType::Artifact => {
          write!(f, "  {}", value.paint(*theme.search_type_label()))?;
        }
        _ => {
          let status_style = status_to_style(value);
          write!(f, "  {}", Badge::new(value, status_style))?;
        }
      }
    }

    // Body content in expanded mode — full content, no truncation
    if self.expanded
      && let Some(ref body) = self.body
    {
      writeln!(f)?;
      writeln!(f)?;
      for line in body.lines() {
        writeln!(f, "    {}", line.paint(*theme.muted()))?;
      }
    }

    Ok(())
  }
}

/// Pick the theme style for a status badge.
///
/// # Arguments
///
/// * `status` - a task or iteration status; `in_progress` and `in-progress` are both accepted
///
/// # Returns
///
/// The matching `status.*` style; an unknown status gets the `status.open` style.
fn status_to_style(status: &str) -> Style {
  let theme = style::global();
  match status {
    "cancelled" => *theme.status_cancelled(),
    "done" => *theme.status_done(),
    "in_progress" | "in-progress" => *theme.status_in_progress(),
    _ => *theme.status_open(),
  }
}
