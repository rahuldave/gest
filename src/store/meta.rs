//! JSON metadata helpers for reading and writing dot-delimited key paths.
//!
//! These helpers operate on [`serde_json::Value`] trees and are shared by all
//! `meta` subcommands across artifacts, iterations, and tasks. They support:
//!
//! * resolving values at a dot path (`outer.inner.leaf`)
//! * inserting values at a dot path, auto-creating intermediate objects
//! * removing values at a dot path
//! * parsing scalar strings into typed JSON values
//! * flattening nested objects into dot-path key/value pairs
//! * formatting values for human display

use serde_json::{Map, Value};

/// Maximum number of dot-delimited segments allowed in a key path.
const MAX_DEPTH: usize = 32;

/// Flatten a JSON value into dot-path key/value pairs for `--raw` output.
///
/// Nested objects are recursively flattened so each leaf becomes a single
/// `key.subkey=value` entry. Non-object roots produce a single empty-key entry.
///
/// # Arguments
///
/// * `root` - the metadata value to flatten
///
/// # Returns
///
/// One `(dot path, formatted value)` pair per leaf, in the map's key order; an empty object adds no pair.
pub fn flatten_dot_paths(root: &Value) -> Vec<(String, String)> {
  let mut out = Vec::new();
  match root {
    Value::Object(map) => flatten_object(map, "", &mut out),
    other => out.push((String::new(), format_meta_value(other))),
  }
  out
}

/// Format a JSON value as a human-friendly string.
///
/// Strings are emitted unquoted; all other types are pretty-printed as JSON.
///
/// # Arguments
///
/// * `value` - the value to format
///
/// # Returns
///
/// The string itself for a JSON string, else the pretty-printed JSON text.
pub fn format_meta_value(value: &Value) -> String {
  match value {
    Value::String(s) => s.clone(),
    other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
  }
}

/// Parse a string into the most specific JSON scalar type.
///
/// Recognizes `true`/`false`, integers, and floats. All other inputs become
/// JSON strings.
///
/// # Arguments
///
/// * `s` - the raw value, as given on the command line
///
/// # Returns
///
/// A JSON boolean for exactly `true` or `false`, a number for an `i64` or a finite `f64`, else a JSON string.
pub fn parse_scalar(s: &str) -> Value {
  match s {
    "true" => return Value::Bool(true),
    "false" => return Value::Bool(false),
    _ => {}
  }
  if let Ok(n) = s.parse::<i64>() {
    return Value::from(n);
  }
  if let Ok(n) = s.parse::<f64>()
    && n.is_finite()
  {
    return Value::from(n);
  }
  Value::String(s.to_string())
}

/// Walk a dot-delimited path through nested objects, returning the leaf value.
///
/// # Arguments
///
/// * `root` - the metadata value to search
/// * `path` - the dot-delimited key path, such as `outer.inner.leaf`
///
/// # Returns
///
/// The value at `path`, or `None` when a key is missing or a segment meets a non-object.
pub fn resolve_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
  path
    .split('.')
    .try_fold(root, |current, seg| current.as_object()?.get(seg))
}

/// Insert a value at a dot-delimited path, creating intermediate objects as needed.
///
/// # Arguments
///
/// * `root` - the metadata object to change
/// * `path` - the dot-delimited key path; a non-object on the way is replaced by an object
/// * `value` - the value to store at the leaf
///
/// # Returns
///
/// Returns `false` and leaves `root` unchanged if `path` exceeds [`MAX_DEPTH`]
/// segments or if `root` is not an object. Otherwise it stores the value and returns `true`.
pub fn set_path(root: &mut Value, path: &str, value: Value) -> bool {
  let segments: Vec<&str> = path.split('.').collect();
  if segments.is_empty() || segments.len() > MAX_DEPTH {
    return false;
  }
  let Some(map) = root.as_object_mut() else {
    return false;
  };
  set_nested(map, &segments, value);
  true
}

/// Remove a value at a dot-delimited path.
///
/// # Arguments
///
/// * `root` - the metadata object to change
/// * `path` - the dot-delimited key path of the value to remove
///
/// # Returns
///
/// Returns `true` if a value was removed, `false` otherwise.
pub fn unset_path(root: &mut Value, path: &str) -> bool {
  let segments: Vec<&str> = path.split('.').collect();
  if segments.is_empty() {
    return false;
  }
  let Some(map) = root.as_object_mut() else {
    return false;
  };
  unset_nested(map, &segments)
}

/// Append one dot-path entry per leaf of `map`, recursing into nested objects.
///
/// # Arguments
///
/// * `map` - the object to flatten
/// * `prefix` - the dot path of `map` itself; empty at the root
/// * `out` - the list that receives each `(dot path, formatted value)` pair
fn flatten_object(map: &Map<String, Value>, prefix: &str, out: &mut Vec<(String, String)>) {
  for (key, value) in map {
    let path = if prefix.is_empty() {
      key.clone()
    } else {
      format!("{prefix}.{key}")
    };
    match value {
      Value::Object(inner) => flatten_object(inner, &path, out),
      other => out.push((path, format_meta_value(other))),
    }
  }
}

/// Store `value` under the key path `segments`, replacing any non-object on the way with an empty object.
///
/// # Arguments
///
/// * `map` - the object at the current depth
/// * `segments` - the remaining keys of the path; nothing happens when it is empty
/// * `value` - the value to store at the last key
fn set_nested(map: &mut Map<String, Value>, segments: &[&str], value: Value) {
  let Some((&first, rest)) = segments.split_first() else {
    return;
  };

  if rest.is_empty() {
    map.insert(first.to_string(), value);
    return;
  }

  let entry = map
    .entry(first.to_string())
    .or_insert_with(|| Value::Object(Map::new()));

  if !entry.is_object() {
    *entry = Value::Object(Map::new());
  }

  if let Some(inner) = entry.as_object_mut() {
    set_nested(inner, rest, value);
  }
}

/// Remove the value under the key path `segments`.
///
/// # Arguments
///
/// * `map` - the object at the current depth
/// * `segments` - the remaining keys of the path
///
/// # Returns
///
/// `true` when the last key existed and was removed; `false` when `segments` is empty, a key is
/// missing or a segment meets a non-object.
fn unset_nested(map: &mut Map<String, Value>, segments: &[&str]) -> bool {
  let Some((&first, rest)) = segments.split_first() else {
    return false;
  };

  if rest.is_empty() {
    return map.remove(first).is_some();
  }

  let Some(next) = map.get_mut(first) else {
    return false;
  };
  let Some(inner) = next.as_object_mut() else {
    return false;
  };
  unset_nested(inner, rest)
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  mod flatten_dot_paths {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flattens_nested_objects() {
      let value = json!({
        "a": 1,
        "b": { "c": "x", "d": { "e": true } },
      });

      let mut pairs = flatten_dot_paths(&value);
      pairs.sort();

      assert_eq!(
        pairs,
        vec![
          ("a".to_string(), "1".to_string()),
          ("b.c".to_string(), "x".to_string()),
          ("b.d.e".to_string(), "true".to_string()),
        ]
      );
    }

    #[test]
    fn it_round_trips_with_set_path() {
      let value = json!({ "outer": { "inner": "deep" }, "flat": 7 });

      let pairs = flatten_dot_paths(&value);
      let mut rebuilt = Value::Object(Map::new());
      for (path, raw) in &pairs {
        assert!(set_path(&mut rebuilt, path, parse_scalar(raw)));
      }

      assert_eq!(rebuilt, value);
    }
  }

  mod format_meta_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_objects_as_pretty_json() {
      let value = json!({ "k": 1 });
      let formatted = format_meta_value(&value);

      assert!(formatted.starts_with('{'));
      assert!(formatted.contains("\"k\""));
    }

    #[test]
    fn it_formats_other_scalars_as_json() {
      assert_eq!(format_meta_value(&json!(42)), "42");
      assert_eq!(format_meta_value(&json!(true)), "true");
    }

    #[test]
    fn it_formats_strings_unquoted() {
      assert_eq!(format_meta_value(&Value::String("hello".to_string())), "hello");
    }
  }

  mod parse_scalar {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_string() {
      assert_eq!(parse_scalar("hello"), Value::String("hello".to_string()));
    }

    #[test]
    fn it_parses_booleans() {
      assert_eq!(parse_scalar("true"), Value::Bool(true));
      assert_eq!(parse_scalar("false"), Value::Bool(false));
    }

    #[test]
    fn it_parses_floats() {
      assert_eq!(parse_scalar("3.14"), json!(3.14));
    }

    #[test]
    fn it_parses_integers() {
      assert_eq!(parse_scalar("42"), json!(42));
    }
  }

  mod resolve_path {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_nested_key() {
      let value = json!({ "outer": { "nested": "deep" } });

      assert_eq!(resolve_path(&value, "outer.nested"), Some(&json!("deep")));
    }

    #[test]
    fn it_resolves_top_level_key() {
      let value = json!({ "key": "value" });

      assert_eq!(resolve_path(&value, "key"), Some(&json!("value")));
    }

    #[test]
    fn it_returns_none_for_missing_key() {
      let value = json!({});

      assert_eq!(resolve_path(&value, "missing"), None);
    }

    #[test]
    fn it_returns_none_when_path_descends_into_scalar() {
      let value = json!({ "leaf": "x" });

      assert_eq!(resolve_path(&value, "leaf.deeper"), None);
    }
  }

  mod set_path {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_creates_intermediate_objects() {
      let mut value = json!({});

      assert!(set_path(&mut value, "a.b.c", json!(1)));

      assert_eq!(value, json!({ "a": { "b": { "c": 1 } } }));
    }

    #[test]
    fn it_overwrites_existing_scalar_with_object() {
      let mut value = json!({ "a": "scalar" });

      assert!(set_path(&mut value, "a.b", json!(2)));

      assert_eq!(value, json!({ "a": { "b": 2 } }));
    }

    #[test]
    fn it_rejects_non_object_root() {
      let mut value = json!("scalar");

      assert!(!set_path(&mut value, "a", json!(1)));
    }

    #[test]
    fn it_rejects_paths_exceeding_max_depth() {
      let mut value = json!({});
      let path = (0..MAX_DEPTH + 1)
        .map(|i| format!("k{i}"))
        .collect::<Vec<_>>()
        .join(".");

      assert!(!set_path(&mut value, &path, json!(1)));
    }

    #[test]
    fn it_sets_top_level_key() {
      let mut value = json!({});

      assert!(set_path(&mut value, "k", json!("v")));

      assert_eq!(value, json!({ "k": "v" }));
    }
  }

  mod unset_path {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_removes_nested_key() {
      let mut value = json!({ "a": { "b": { "c": 1 }, "d": 2 } });

      assert!(unset_path(&mut value, "a.b.c"));

      assert_eq!(value, json!({ "a": { "b": {}, "d": 2 } }));
    }

    #[test]
    fn it_removes_top_level_key() {
      let mut value = json!({ "a": 1, "b": 2 });

      assert!(unset_path(&mut value, "a"));

      assert_eq!(value, json!({ "b": 2 }));
    }

    #[test]
    fn it_returns_false_when_intermediate_missing() {
      let mut value = json!({ "a": 1 });

      assert!(!unset_path(&mut value, "b.c"));
    }

    #[test]
    fn it_returns_false_when_key_missing() {
      let mut value = json!({});

      assert!(!unset_path(&mut value, "missing"));
    }
  }
}
