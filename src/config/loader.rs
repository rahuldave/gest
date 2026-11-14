//! Finding, reading and merging the layered config files and `GEST_` environment variables into [`Settings`].

use std::{env, fs, path::PathBuf};

use toml::{Value, value::Table};
use typed_env::{EnvarParse, EnvarParser};

use super::{Error, Settings, env::GEST_CONFIG};

/// Return the active global config file path, if one exists on disk.
///
/// # Returns
///
/// The path from `$GEST_CONFIG` or the XDG default, or `None` when no file is there or the path cannot be resolved.
pub fn active_global_config_path() -> Option<PathBuf> {
  let path = global_config_path().ok()?;
  path.is_file().then_some(path)
}

/// Return the active per-directory config file paths in load order, walking from
/// filesystem root down to `$CWD`. Only files that exist on disk are included.
///
/// # Returns
///
/// The paths; empty when there are none.
pub fn active_project_config_paths() -> Vec<PathBuf> {
  let cwd = env::current_dir().unwrap_or_default();
  let mut ancestors: Vec<PathBuf> = cwd.ancestors().map(PathBuf::from).collect();
  ancestors.reverse();

  let mut paths = Vec::new();
  for dir in ancestors {
    for candidate in [dir.join(".config/gest.toml"), dir.join(".gest.toml")] {
      if candidate.is_file() {
        paths.push(candidate);
      }
    }
  }
  paths
}

/// Load and merge configuration from all sources.
///
/// Resolution order (each layer overrides the previous):
///
/// 1. Global config: `$GEST_CONFIG` or `$XDG_CONFIG_HOME/gest/config.toml`
/// 2. Per-directory configs walking from filesystem root to `$CWD`, checking `.config/gest.toml` and `.gest.toml` at
///    each level.
/// 3. Environment variables prefixed with `GEST_` (e.g. `GEST_STORAGE__DATA_DIR`).
///
/// # Returns
///
/// The merged settings, or an error when a file cannot be read, holds invalid TOML or does not match [`Settings`],
/// or when the XDG config directory cannot be resolved.
pub fn load() -> Result<Settings, Error> {
  let mut merged = Table::new();

  let global = global_config_path()?;
  if let Some(table) = read_toml(&global)? {
    log::debug!("config: merged global {}", global.display());
    merge_tables(&mut merged, table);
  }

  let cwd = env::current_dir().unwrap_or_default();
  let mut ancestors: Vec<PathBuf> = cwd.ancestors().map(PathBuf::from).collect();
  ancestors.reverse();

  for dir in ancestors {
    for candidate in [dir.join(".config/gest.toml"), dir.join(".gest.toml")] {
      if let Some(table) = read_toml(&candidate)? {
        log::debug!("config: merged {}", candidate.display());
        merge_tables(&mut merged, table);
      }
    }
  }

  merge_env(&mut merged);

  let settings: Settings = Value::Table(merged).try_into()?;

  Ok(settings)
}

/// Resolve the path to the global config file, preferring `$GEST_CONFIG` over the XDG default.
///
/// # Returns
///
/// The path, which may not exist, or [`Error::XDGDirNotFound`] when `$GEST_CONFIG` is unset and the XDG config
/// directory cannot be resolved.
fn global_config_path() -> Result<PathBuf, Error> {
  GEST_CONFIG
    .value()
    .ok()
    .or_else(|| dir_spec::config_home().map(|path| path.join("gest/config.toml")))
    .ok_or(Error::XDGDirNotFound("config"))
}

/// Merge environment variables prefixed with `GEST_` into the config table.
///
/// `__` separates nesting levels and key segments are lowercased.
/// For example, `GEST_STORAGE__DATA_DIR=foo` becomes `[storage] data_dir = "foo"`.
/// `GEST_CONFIG` itself is skipped.
///
/// A value is inserted as a string when [`Settings`] accepts a string at that key, and otherwise as a bool, an
/// integer or a float when it parses as one, so `GEST_STORAGE__SYNC=false` sets the bool `storage.sync`. A bool
/// accepts the words that [`typed_env`] accepts, such as `true`, `1`, `yes` and `on`, so the config value agrees with
/// the variable read directly by [`super::storage::Settings::sync_enabled`]. A value that
/// fits no type is inserted as a string, so loading reports the bad value. A variable that names a table, such as
/// `GEST_PAGER` for `[pager]`, or that nests under a key that is not a table, such as `GEST_STORAGE__SYNC__X`, is not
/// a config key and is skipped.
///
/// # Arguments
///
/// * `base` - the merged table, changed in place
fn merge_env(base: &mut Table) {
  const PREFIX: &str = "GEST_";

  let defaults = Value::try_from(Settings::default()).unwrap_or_else(|_| Value::Table(Table::new()));

  for (key, value) in env::vars() {
    if !key.starts_with(PREFIX) || key == GEST_CONFIG.name() {
      continue;
    }

    let path: Vec<String> = key[PREFIX.len()..].split("__").map(str::to_lowercase).collect();

    if lookup(&defaults, &path).is_some_and(Value::is_table) {
      log::debug!("config: {key} names a config table, not a value; skipped");
      continue;
    }

    let parents = &path[..path.len() - 1];
    let nests_under_value = (1..=parents.len()).any(|depth| {
      let prefix = &parents[..depth];
      !fits(prefix, &Value::Table(Table::new())) || lookup_table(base, prefix).is_some_and(|found| !found.is_table())
    });
    if nests_under_value {
      log::debug!("config: {key} nests under a key that is not a table; skipped");
      continue;
    }

    log::trace!("config: env override {key}");
    let value = typed_env_value(&path, value);
    let mut table = &mut *base;
    for segment in parents {
      table = table
        .entry(segment.clone())
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("checked above that every parent is a table");
    }
    table.insert(path.last().unwrap().clone(), value);
  }
}

/// Return the value at `path` inside `value`, if every segment exists.
fn lookup<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
  path
    .iter()
    .try_fold(value, |current, segment| current.as_table()?.get(segment))
}

/// Return the value at the non-empty `path` inside `table`, if every segment exists.
fn lookup_table<'a>(table: &'a Table, path: &[String]) -> Option<&'a Value> {
  let (first, rest) = path.split_first()?;
  lookup(table.get(first)?, rest)
}

/// Give a `GEST_*` value the type that [`Settings`] expects at `path`.
///
/// # Returns
///
/// A string when a string fits, else the first of bool, integer or float that the text parses as and that fits, else
/// the string, so deserialization reports the mismatch.
fn typed_env_value(path: &[String], raw: String) -> Value {
  let string = Value::String(raw);
  if fits(path, &string) {
    return string;
  }
  let Value::String(raw) = &string else { unreachable!() };
  let candidates = [
    <EnvarParser<bool> as EnvarParse<bool>>::parse("GEST_".into(), raw)
      .ok()
      .map(Value::Boolean),
    raw.parse::<i64>().ok().map(Value::Integer),
    raw.parse::<f64>().ok().map(Value::Float),
  ];
  candidates
    .into_iter()
    .flatten()
    .find(|candidate| fits(path, candidate))
    .unwrap_or(string)
}

thread_local! {
  static PROBING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether this thread is probing a value with [`fits`], so deserializers should not print warnings.
pub(super) fn probing() -> bool {
  PROBING.with(std::cell::Cell::get)
}

/// Whether [`Settings`] deserializes from a table holding only `value` at `path`.
///
/// Deserializers print no warnings during the probe; the real load prints them once.
fn fits(path: &[String], value: &Value) -> bool {
  let nested = path.iter().rev().fold(value.clone(), |inner, segment| {
    Value::Table(Table::from_iter([(segment.clone(), inner)]))
  });
  PROBING.with(|flag| flag.set(true));
  let ok = nested.try_into::<Settings>().is_ok();
  PROBING.with(|flag| flag.set(false));
  ok
}

/// Recursively merge `overlay` into `base`. Nested tables are merged; all other values are overwritten.
///
/// # Arguments
///
/// * `base` - the table to merge into, changed in place
/// * `overlay` - the table whose values win
fn merge_tables(base: &mut Table, overlay: Table) {
  for (key, overlay_value) in overlay {
    if let Value::Table(overlay_table) = &overlay_value
      && let Some(Value::Table(base_table)) = base.get_mut(&key)
    {
      merge_tables(base_table, overlay_table.clone());
      continue;
    }
    base.insert(key, overlay_value);
  }
}

/// Read and parse a TOML file into a [`Table`]. Returns `Ok(None)` if the file does not exist.
///
/// # Arguments
///
/// * `path` - the config file to read
///
/// # Returns
///
/// The parsed table, or an error when the file cannot be read or is invalid TOML.
fn read_toml(path: &PathBuf) -> Result<Option<Table>, Error> {
  if !path.is_file() {
    return Ok(None);
  }

  let content = fs::read_to_string(path)?;
  let table: Table = toml::from_str(&content)?;

  Ok(Some(table))
}

#[cfg(test)]
mod tests {
  use std::io::Write;

  use tempfile::TempDir;
  use toml::Value;

  use super::*;

  mod load {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::logging::LevelFilter;

    #[test]
    fn it_merges_per_directory_config_into_settings() {
      let dir = TempDir::new().unwrap();
      let config_path = dir.path().join(".gest.toml");

      let mut file = fs::File::create(&config_path).unwrap();
      file.write_all(b"[storage]\ndata_dir = \"/custom/data\"\n").unwrap();

      temp_env::with_vars(
        [
          ("GEST_CONFIG", None::<&str>),
          ("GEST_DATA_DIR", None::<&str>),
          ("XDG_CONFIG_HOME", Some(dir.path().join(".config").to_str().unwrap())),
        ],
        || {
          let original_dir = env::current_dir().unwrap();
          env::set_current_dir(dir.path()).unwrap();

          let settings = load().unwrap();

          env::set_current_dir(original_dir).unwrap();

          assert_eq!(
            settings.storage().data_dir().unwrap(),
            std::path::PathBuf::from("/custom/data")
          );
        },
      );
    }

    /// Run `load` in an empty directory with no global config and the given `GEST_*` variables set.
    fn load_with_env(vars: &[(&str, &str)]) -> Result<Settings, Error> {
      let dir = TempDir::new().unwrap();
      let config_home = dir.path().join(".config");
      let mut all: Vec<(String, Option<String>)> = env::vars()
        .filter(|(key, _)| key.starts_with("GEST_"))
        .map(|(key, _)| (key, None))
        .collect();
      all.push(("XDG_CONFIG_HOME".into(), Some(config_home.to_str().unwrap().into())));
      for (key, value) in vars {
        all.push(((*key).into(), Some((*value).into())));
      }

      temp_env::with_vars(all, || {
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(dir.path()).unwrap();
        let result = load();
        env::set_current_dir(original_dir).unwrap();
        result
      })
    }

    #[test]
    fn it_reads_a_bool_from_the_environment() {
      for (raw, expected) in [("false", false), ("true", true), ("off", false), ("1", true)] {
        let settings = load_with_env(&[("GEST_STORAGE__SYNC", raw)]).unwrap();
        let storage = Value::try_from(settings.storage()).unwrap();

        assert_eq!(
          storage.get("sync"),
          Some(&Value::Boolean(expected)),
          "GEST_STORAGE__SYNC={raw}"
        );
      }
    }

    #[test]
    fn it_reads_an_integer_from_the_environment() {
      let settings = load_with_env(&[("GEST_SERVE__PORT", "8123")]).unwrap();

      assert_eq!(settings.serve().port(), 8123);
    }

    #[test]
    fn it_keeps_a_string_value_that_looks_like_a_number() {
      let settings = load_with_env(&[("GEST_PAGER__COMMAND", "42")]).unwrap();

      assert_eq!(settings.pager().command(), Some("42"));
    }

    #[test]
    fn it_ignores_gest_pager_which_names_a_table() {
      let settings = load_with_env(&[("GEST_PAGER", "less")]).unwrap();

      assert_eq!(settings.pager(), &crate::config::pager::Settings::default());
    }

    #[test]
    fn it_ignores_a_variable_that_nests_under_a_value() {
      let settings = load_with_env(&[("GEST_SERVE__PORT__EXTRA", "1")]).unwrap();

      assert_eq!(settings.serve().port(), 2300);
    }

    #[test]
    fn it_reads_a_log_level_in_any_case_or_as_a_number() {
      for (raw, expected) in [
        ("DEBUG", LevelFilter::Debug),
        ("3", LevelFilter::Info),
        ("trace", LevelFilter::Trace),
      ] {
        let settings = load_with_env(&[("GEST_LOG__LEVEL", raw), ("GEST_SERVE__LOG_LEVEL", raw)]).unwrap();

        assert_eq!(settings.log().level(), expected, "GEST_LOG__LEVEL={raw}");
        assert_eq!(
          settings.serve().log_level(),
          Some(expected),
          "GEST_SERVE__LOG_LEVEL={raw}"
        );
      }
    }

    #[test]
    fn it_ignores_a_variable_that_nests_under_a_field_that_defaults_to_none() {
      for key in [
        "GEST_STORAGE__SYNC__X",
        "GEST_DATABASE__URL__X",
        "GEST_PAGER__COMMAND__X",
        "GEST_SERVE__LOG_LEVEL__X",
      ] {
        assert!(load_with_env(&[(key, "1")]).is_ok(), "{key}=1 broke loading");
      }
    }

    #[test]
    fn it_accepts_an_empty_table_for_every_section() {
      let defaults = Value::try_from(Settings::default()).unwrap();
      for section in defaults.as_table().unwrap().keys() {
        assert!(
          fits(std::slice::from_ref(section), &Value::Table(Table::new())),
          "[{section}] rejects an empty table"
        );
      }
    }

    #[test]
    fn it_still_rejects_a_value_of_the_wrong_type() {
      assert!(load_with_env(&[("GEST_STORAGE__SYNC", "maybe")]).is_err());
    }
  }

  mod merge_env {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_inserts_a_flat_key() {
      temp_env::with_var("GEST_TESTFLAT", Some("hello"), || {
        let mut table = Table::new();
        merge_env(&mut table);

        assert_eq!(table.get("testflat").unwrap().as_str().unwrap(), "hello");
      });
    }

    #[test]
    fn it_inserts_a_nested_key() {
      temp_env::with_var("GEST_SECTION__NESTED_KEY", Some("val"), || {
        let mut table = Table::new();
        merge_env(&mut table);

        let section = table.get("section").unwrap().as_table().unwrap();
        assert_eq!(section.get("nested_key").unwrap().as_str().unwrap(), "val");
      });
    }

    #[test]
    fn it_lowercases_key_segments() {
      temp_env::with_var("GEST_UPPER__CASE", Some("low"), || {
        let mut table = Table::new();
        merge_env(&mut table);

        let section = table.get("upper").unwrap().as_table().unwrap();
        assert_eq!(section.get("case").unwrap().as_str().unwrap(), "low");
      });
    }

    #[test]
    fn it_skips_gest_config_env_var() {
      temp_env::with_var("GEST_CONFIG", Some("/some/path"), || {
        let mut table = Table::new();
        merge_env(&mut table);

        assert!(table.get("config").is_none());
      });
    }
  }

  mod merge_tables {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_inserts_new_keys() {
      let mut base = Table::new();
      let mut overlay = Table::new();
      overlay.insert("key".into(), Value::String("value".into()));

      merge_tables(&mut base, overlay);

      assert_eq!(base.get("key").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn it_overwrites_scalar_values() {
      let mut base = Table::new();
      base.insert("key".into(), Value::String("old".into()));

      let mut overlay = Table::new();
      overlay.insert("key".into(), Value::String("new".into()));

      merge_tables(&mut base, overlay);

      assert_eq!(base.get("key").unwrap().as_str().unwrap(), "new");
    }

    #[test]
    fn it_recursively_merges_nested_tables() {
      let mut inner_base = Table::new();
      inner_base.insert("a".into(), Value::String("1".into()));
      inner_base.insert("b".into(), Value::String("2".into()));

      let mut base = Table::new();
      base.insert("section".into(), Value::Table(inner_base));

      let mut inner_overlay = Table::new();
      inner_overlay.insert("b".into(), Value::String("overridden".into()));
      inner_overlay.insert("c".into(), Value::String("3".into()));

      let mut overlay = Table::new();
      overlay.insert("section".into(), Value::Table(inner_overlay));

      merge_tables(&mut base, overlay);

      let section = base.get("section").unwrap().as_table().unwrap();
      assert_eq!(section.get("a").unwrap().as_str().unwrap(), "1");
      assert_eq!(section.get("b").unwrap().as_str().unwrap(), "overridden");
      assert_eq!(section.get("c").unwrap().as_str().unwrap(), "3");
    }

    #[test]
    fn it_replaces_scalar_with_table() {
      let mut base = Table::new();
      base.insert("key".into(), Value::String("scalar".into()));

      let mut inner = Table::new();
      inner.insert("nested".into(), Value::String("value".into()));

      let mut overlay = Table::new();
      overlay.insert("key".into(), Value::Table(inner));

      merge_tables(&mut base, overlay);

      let table = base.get("key").unwrap().as_table().unwrap();
      assert_eq!(table.get("nested").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn it_replaces_table_with_scalar() {
      let mut inner = Table::new();
      inner.insert("nested".into(), Value::String("value".into()));

      let mut base = Table::new();
      base.insert("key".into(), Value::Table(inner));

      let mut overlay = Table::new();
      overlay.insert("key".into(), Value::String("scalar".into()));

      merge_tables(&mut base, overlay);

      assert_eq!(base.get("key").unwrap().as_str().unwrap(), "scalar");
    }
  }

  mod read_toml {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_a_valid_toml_file() {
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("config.toml");

      let mut file = fs::File::create(&path).unwrap();
      file.write_all(b"[section]\nkey = \"value\"\n").unwrap();

      let result = read_toml(&path).unwrap().unwrap();

      let section = result.get("section").unwrap().as_table().unwrap();
      assert_eq!(section.get("key").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn it_returns_an_error_for_invalid_toml() {
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("bad.toml");

      let mut file = fs::File::create(&path).unwrap();
      file.write_all(b"not valid [[ toml").unwrap();

      let result = read_toml(&path);

      assert!(result.is_err());
    }

    #[test]
    fn it_returns_none_for_missing_file() {
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("nonexistent.toml");

      let result = read_toml(&path).unwrap();

      assert!(result.is_none());
    }
  }
}
