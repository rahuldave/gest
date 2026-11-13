//! Aligned key-value field list composed from [`Label`] and [`Value`] atoms.

use std::fmt::{self, Display, Formatter};

use crate::ui::{
  components::atoms::{Label, Value},
  style,
};

/// Renders a list of label-value pairs with labels right-padded for column alignment.
///
/// Each row is indented by two spaces, with two spaces separating the label and value.
/// Pre-styled values bypass the default [`Value`] styling.
pub struct Component {
  /// Rows in insertion order.
  fields: Vec<Field>,
}

impl Component {
  /// Create an empty field list.
  ///
  /// # Returns
  ///
  /// A list that displays as nothing until fields are added.
  pub fn new() -> Self {
    Self {
      fields: Vec::new(),
    }
  }

  /// Append a label-value pair.
  ///
  /// # Arguments
  ///
  /// * `label` - the field name, shown muted
  /// * `value` - the field value, shown in the `config.value` style
  ///
  /// # Returns
  ///
  /// The list with the field added.
  pub fn field(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
    self.fields.push(Field {
      label: label.into(),
      styled: false,
      value: value.into(),
    });
    self
  }

  /// Append a pre-styled label-value pair that bypasses default value styling.
  ///
  /// # Arguments
  ///
  /// * `label` - the field name, shown muted
  /// * `value` - the value, rendered now and printed with its own styling
  ///
  /// # Returns
  ///
  /// The list with the field added.
  pub fn styled_field(mut self, label: impl Into<String>, value: impl Display) -> Self {
    self.fields.push(Field {
      label: label.into(),
      styled: true,
      value: value.to_string(),
    });
    self
  }
}

/// One row of a field list, kept until display so labels can be aligned.
struct Field {
  /// Label text, padded to the longest label in the list.
  label: String,
  /// Whether `value` is already styled and is printed as is, not in the `config.value` style.
  styled: bool,
  /// Value text, or its rendered form for a pre-styled field.
  value: String,
}

impl Display for Component {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    if self.fields.is_empty() {
      return Ok(());
    }

    let theme = style::global();
    let max_label = self.fields.iter().map(|f| f.label.len()).max().unwrap_or(0);

    for (i, field) in self.fields.iter().enumerate() {
      if i > 0 {
        writeln!(f)?;
      }

      let label = Label::new(&field.label, *theme.muted()).pad_to(max_label);

      if field.styled {
        write!(f, "  {label}  {}", field.value)?;
      } else {
        write!(f, "  {label}  {}", Value::new(&field.value, *theme.config_value()))?;
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod fmt {
    use super::*;

    #[test]
    fn it_aligns_labels_across_fields() {
      let component = Component::new()
        .field("title", "probe-schema-v2")
        .field("source", "probe-schema-v2.md");
      let output = component.to_string();

      let lines: Vec<&str> = output.lines().collect();
      assert_eq!(lines.len(), 2);
      assert!(output.contains("title"));
      assert!(output.contains("source"));
      assert!(output.contains("probe-schema-v2"));
      assert!(output.contains("probe-schema-v2.md"));
    }

    #[test]
    fn it_pads_shorter_labels_to_match_longest() {
      let component = Component::new().field("id", "abc123").field("data dir", ".gest/");
      let output = component.to_string();

      let lines: Vec<&str> = output.lines().collect();
      assert_eq!(lines.len(), 2);

      // Strip ANSI to check structure
      let plain: String = output.chars().filter(|c| !c.is_ascii_control()).collect();
      let plain = plain.replace("[0m", "").replace("[1m", "");
      assert!(plain.contains("id        "), "short label should be padded");
    }

    #[test]
    fn it_renders_nothing_when_empty() {
      let component = Component::new();

      assert_eq!(component.to_string(), "");
    }

    #[test]
    fn it_renders_styled_field_values_without_theme_styling() {
      let component = Component::new().styled_field("status", "open");

      let output = component.to_string();

      assert!(output.contains("status"));
      assert!(output.contains("open"));
    }
  }
}
