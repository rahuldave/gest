//! `gest config get`: get a configuration value by dotted key path.

use clap::Args;
use toml::Value;

use crate::{AppContext, cli::Error, ui::json};

/// Get a configuration value by dotted key path.
#[derive(Args, Debug)]
pub struct Command {
  /// The configuration key (e.g. "storage.data_dir", "log.level").
  key: String,
  /// The output mode: normal, `--json`, `--quiet` or `--raw`.
  #[command(flatten)]
  output: json::Flags,
}

impl Command {
  /// Resolve `key` against the merged settings and print its value.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("config get: entry");
    let toml_value = Value::try_from(context.settings()).map_err(std::io::Error::other)?;
    let value = resolve_dotted_key(&toml_value, &self.key).cloned();

    let human = value
      .as_ref()
      .map_or_else(|| "(not set)".to_string(), display_toml_value);
    self.output.print_entity(&value, &human, || human.clone())?;
    Ok(())
  }
}

/// Format a TOML value for the human output: strings bare, everything else as TOML.
///
/// # Arguments
///
/// * `value` - the value to show
///
/// # Returns
///
/// A string without quotes, or the TOML text of any other value.
fn display_toml_value(value: &Value) -> String {
  match value {
    Value::String(s) => s.clone(),
    other => other.to_string(),
  }
}

/// Walk a TOML value down a dotted key such as `log.level`.
///
/// # Arguments
///
/// * `value` - the root of the settings, as TOML
/// * `key` - the dot-delimited key
///
/// # Returns
///
/// The value at `key`, or `None` when any part of the path is missing.
fn resolve_dotted_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
  let mut current = value;
  for part in key.split('.') {
    current = current.get(part)?;
  }
  Some(current)
}
