//! Write-side helpers for tombstoning on-disk entity files.
//!
//! When a project is in local sync mode, deleting an entity in SQLite must
//! also mark its on-disk file as tombstoned so the next import does not
//! resurrect the row. These helpers locate the per-entity file under the
//! gest folder layout (ADR-0016), parse it generically via
//! [`yaml_serde::Value`] so that unknown or non-scalar fields are preserved
//! verbatim, inject `deleted_at`, and rewrite the file in place.
//!
//! The file itself is never removed — it *is* the tombstone and is what
//! downstream clones will read to learn about the deletion.

use std::{fs, path::Path};

use chrono::{DateTime, Utc};
use yaml_serde::{Mapping, Value as YamlValue};

use crate::store::{Error, model::primitives::Id, sync::paths};

/// The line that opens and closes the YAML frontmatter of an artifact's markdown file.
const FRONTMATTER_DELIM: &str = "---\n";

/// Tombstone the on-disk file for an artifact by setting `deleted_at` in its
/// YAML frontmatter. No-op if `gest_dir` is `None` or the file is missing.
///
/// The markdown body after the frontmatter is kept.
///
/// # Arguments
///
/// * `gest_dir` - the project's `.gest/` directory, or `None` when the project does not sync
/// * `id` - the artifact's id, which locates `artifact/<id>.md`
/// * `deleted_at` - the deletion time, written as RFC 3339
///
/// # Returns
///
/// `Ok(())`, or an error when the frontmatter delimiters are missing, or an I/O or YAML error when the file
/// cannot be read, parsed or rewritten.
pub fn tombstone_artifact(gest_dir: Option<&Path>, id: &Id, deleted_at: DateTime<Utc>) -> Result<(), Error> {
  let Some(gest_dir) = gest_dir else {
    return Ok(());
  };
  let path = paths::artifact_path(gest_dir, id);
  if !path.exists() {
    return Ok(());
  }
  let raw = fs::read_to_string(&path)?;
  let (front, body) = split_frontmatter(&raw)?;
  let mut value: YamlValue = yaml_serde::from_str(front)?;
  set_deleted_at(&mut value, deleted_at)?;
  let new_front = yaml_serde::to_string(&value)?;
  let composed = compose_frontmatter(&new_front, body);
  fs::write(&path, composed)?;
  Ok(())
}

/// Tombstone the on-disk file for an iteration. No-op if `gest_dir` is
/// `None` or the file is missing.
///
/// # Arguments
///
/// * `gest_dir` - the project's `.gest/` directory, or `None` when the project does not sync
/// * `id` - the iteration's id, which locates `iteration/<id>.yaml`
/// * `deleted_at` - the deletion time, written as RFC 3339
///
/// # Returns
///
/// `Ok(())`, or an I/O or YAML error when the file cannot be read, parsed or rewritten.
pub fn tombstone_iteration(gest_dir: Option<&Path>, id: &Id, deleted_at: DateTime<Utc>) -> Result<(), Error> {
  tombstone_yaml(gest_dir.map(|d| paths::iteration_path(d, id)).as_deref(), deleted_at)
}

/// Tombstone the on-disk `project.yaml` file. No-op if `gest_dir` is `None`
/// or the file is missing.
///
/// # Arguments
///
/// * `gest_dir` - the project's `.gest/` directory, or `None` when the project does not sync
/// * `deleted_at` - the deletion time, written as RFC 3339
///
/// # Returns
///
/// `Ok(())`, or an I/O or YAML error when the file cannot be read, parsed or rewritten.
pub fn tombstone_project(gest_dir: Option<&Path>, deleted_at: DateTime<Utc>) -> Result<(), Error> {
  tombstone_yaml(gest_dir.map(paths::project_path).as_deref(), deleted_at)
}

/// Tombstone the on-disk file for a task. No-op if `gest_dir` is `None` or
/// the file is missing.
///
/// # Arguments
///
/// * `gest_dir` - the project's `.gest/` directory, or `None` when the project does not sync
/// * `id` - the task's id, which locates `task/<id>.yaml`
/// * `deleted_at` - the deletion time, written as RFC 3339
///
/// # Returns
///
/// `Ok(())`, or an I/O or YAML error when the file cannot be read, parsed or rewritten.
pub fn tombstone_task(gest_dir: Option<&Path>, id: &Id, deleted_at: DateTime<Utc>) -> Result<(), Error> {
  tombstone_yaml(gest_dir.map(|d| paths::task_path(d, id)).as_deref(), deleted_at)
}

/// Rebuild a markdown file from its frontmatter and its body.
///
/// # Arguments
///
/// * `front` - the YAML frontmatter, without its `---` lines
/// * `body` - the markdown after the closing `---` line
///
/// # Returns
///
/// `---`, the frontmatter, `---` and the body, with a newline added after the frontmatter and the
/// body when either lacks one.
fn compose_frontmatter(front: &str, body: &str) -> String {
  let mut out = String::new();
  out.push_str(FRONTMATTER_DELIM);
  out.push_str(front);
  if !front.ends_with('\n') {
    out.push('\n');
  }
  out.push_str(FRONTMATTER_DELIM);
  out.push_str(body);
  if !body.ends_with('\n') {
    out.push('\n');
  }
  out
}

/// Insert or overwrite the `deleted_at` key of a YAML document.
///
/// A document that is not a mapping is replaced by a mapping that holds only `deleted_at`.
///
/// # Arguments
///
/// * `value` - the parsed YAML document to change in place
/// * `deleted_at` - the deletion time, stored as an RFC 3339 string
///
/// # Returns
///
/// Always `Ok(())`.
fn set_deleted_at(value: &mut YamlValue, deleted_at: DateTime<Utc>) -> Result<(), Error> {
  if value.as_mapping().is_none() {
    *value = YamlValue::Mapping(Mapping::new());
  }
  let mapping = value
    .as_mapping_mut()
    .expect("value is a mapping after the guard above");
  mapping.insert(
    YamlValue::String("deleted_at".into()),
    YamlValue::String(deleted_at.to_rfc3339()),
  );
  Ok(())
}

/// Split a markdown file into its YAML frontmatter and its body.
///
/// # Arguments
///
/// * `raw` - the whole file, which must start with a `---` line
///
/// # Returns
///
/// The frontmatter, with its final newline, and the body after the first closing `---` and one
/// newline; an I/O error when the opening or closing delimiter is missing.
fn split_frontmatter(raw: &str) -> Result<(&str, &str), Error> {
  let trimmed = raw.strip_prefix(FRONTMATTER_DELIM).ok_or_else(|| {
    Error::Io(std::io::Error::other(
      "artifact file is missing the leading `---` frontmatter delimiter",
    ))
  })?;
  let end = trimmed
    .find("\n---")
    .ok_or_else(|| Error::Io(std::io::Error::other("artifact frontmatter is not closed with `---`")))?;
  let front = &trimmed[..end + 1];
  let after_delim = &trimmed[end + 4..];
  let body = after_delim.strip_prefix('\n').unwrap_or(after_delim);
  Ok((front, body))
}

/// Set `deleted_at` in a plain YAML entity file and rewrite it.
///
/// Other keys are kept. No-op when `path` is `None` or the file is missing.
///
/// # Arguments
///
/// * `path` - the entity's YAML file, or `None` when the project does not sync
/// * `deleted_at` - the deletion time, written as RFC 3339
///
/// # Returns
///
/// `Ok(())`, or an I/O or YAML error when the file cannot be read, parsed or rewritten.
fn tombstone_yaml(path: Option<&Path>, deleted_at: DateTime<Utc>) -> Result<(), Error> {
  let Some(path) = path else { return Ok(()) };
  if !path.exists() {
    return Ok(());
  }
  let raw = fs::read_to_string(path)?;
  let mut value: YamlValue = yaml_serde::from_str(&raw)?;
  set_deleted_at(&mut value, deleted_at)?;
  let mut out = yaml_serde::to_string(&value)?;
  if !out.ends_with('\n') {
    out.push('\n');
  }
  fs::write(path, out)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone;
  use tempfile::TempDir;

  use super::*;

  fn sample_deleted_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 8, 12, 0, 0).unwrap()
  }

  fn sample_id() -> Id {
    "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk".parse().unwrap()
  }

  mod tombstone_artifact {
    use super::*;

    #[test]
    fn it_is_a_noop_when_gest_dir_is_none() {
      tombstone_artifact(None, &sample_id(), sample_deleted_at()).unwrap();
    }

    #[test]
    fn it_is_a_noop_when_file_is_missing() {
      let tmp = TempDir::new().unwrap();

      tombstone_artifact(Some(tmp.path()), &sample_id(), sample_deleted_at()).unwrap();

      assert!(!paths::artifact_path(tmp.path(), &sample_id()).exists());
    }

    #[test]
    fn it_sets_deleted_at_and_preserves_body_and_other_fields() {
      let tmp = TempDir::new().unwrap();
      let id = sample_id();
      let path = paths::artifact_path(tmp.path(), &id);
      fs::create_dir_all(path.parent().unwrap()).unwrap();
      fs::write(
        &path,
        "---\nid: kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\ntitle: Spec\ntags:\n- design\ncreated_at: 2026-04-01T00:00:00Z\nupdated_at: 2026-04-01T00:00:00Z\n---\n# Heading\n\nBody text.\n",
      )
      .unwrap();

      tombstone_artifact(Some(tmp.path()), &id, sample_deleted_at()).unwrap();

      let raw = fs::read_to_string(&path).unwrap();
      assert!(raw.contains("deleted_at: 2026-04-08T12:00:00+00:00"));
      assert!(raw.contains("title: Spec"));
      assert!(raw.contains("- design"));
      assert!(raw.contains("# Heading"));
      assert!(raw.contains("Body text."));
    }
  }

  mod tombstone_iteration {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_a_noop_when_gest_dir_is_none() {
      tombstone_iteration(None, &sample_id(), sample_deleted_at()).unwrap();
    }

    #[test]
    fn it_is_a_noop_when_file_is_missing() {
      let tmp = TempDir::new().unwrap();

      tombstone_iteration(Some(tmp.path()), &sample_id(), sample_deleted_at()).unwrap();
    }

    #[test]
    fn it_sets_deleted_at_and_preserves_other_fields() {
      let tmp = TempDir::new().unwrap();
      let id = sample_id();
      let path = paths::iteration_path(tmp.path(), &id);
      fs::create_dir_all(path.parent().unwrap()).unwrap();
      fs::write(
        &path,
        "id: kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\ntitle: Sprint\nstatus: active\ncreated_at: 2026-04-01T00:00:00Z\nupdated_at: 2026-04-01T00:00:00Z\n",
      )
      .unwrap();

      tombstone_iteration(Some(tmp.path()), &id, sample_deleted_at()).unwrap();

      let value: YamlValue = yaml_serde::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
      let mapping = value.as_mapping().unwrap();
      assert_eq!(
        mapping
          .get(YamlValue::String("deleted_at".into()))
          .and_then(YamlValue::as_str),
        Some("2026-04-08T12:00:00+00:00")
      );
      assert_eq!(
        mapping
          .get(YamlValue::String("title".into()))
          .and_then(YamlValue::as_str),
        Some("Sprint")
      );
      assert_eq!(
        mapping
          .get(YamlValue::String("status".into()))
          .and_then(YamlValue::as_str),
        Some("active")
      );
    }
  }

  mod tombstone_project {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_a_noop_when_gest_dir_is_none() {
      tombstone_project(None, sample_deleted_at()).unwrap();
    }

    #[test]
    fn it_is_a_noop_when_file_is_missing() {
      let tmp = TempDir::new().unwrap();

      tombstone_project(Some(tmp.path()), sample_deleted_at()).unwrap();
    }

    #[test]
    fn it_sets_deleted_at_and_preserves_other_fields() {
      let tmp = TempDir::new().unwrap();
      let path = paths::project_path(tmp.path());
      fs::write(
        &path,
        "id: kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\ncreated_at: 2026-04-01T00:00:00Z\nupdated_at: 2026-04-01T00:00:00Z\n",
      )
      .unwrap();

      tombstone_project(Some(tmp.path()), sample_deleted_at()).unwrap();

      let value: YamlValue = yaml_serde::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
      let mapping = value.as_mapping().unwrap();
      assert_eq!(
        mapping
          .get(YamlValue::String("deleted_at".into()))
          .and_then(YamlValue::as_str),
        Some("2026-04-08T12:00:00+00:00")
      );
      assert_eq!(
        mapping.get(YamlValue::String("id".into())).and_then(YamlValue::as_str),
        Some("kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk")
      );
    }
  }

  mod tombstone_task {
    use super::*;

    #[test]
    fn it_is_a_noop_when_gest_dir_is_none() {
      tombstone_task(None, &sample_id(), sample_deleted_at()).unwrap();
    }

    #[test]
    fn it_is_a_noop_when_file_is_missing() {
      let tmp = TempDir::new().unwrap();

      tombstone_task(Some(tmp.path()), &sample_id(), sample_deleted_at()).unwrap();
    }

    #[test]
    fn it_sets_deleted_at_and_preserves_other_fields() {
      let tmp = TempDir::new().unwrap();
      let id = sample_id();
      let path = paths::task_path(tmp.path(), &id);
      fs::create_dir_all(path.parent().unwrap()).unwrap();
      fs::write(
        &path,
        "id: kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\ntitle: Fix bug\nstatus: open\ndescription: detail\ntags:\n- bug\ncreated_at: 2026-04-01T00:00:00Z\nupdated_at: 2026-04-01T00:00:00Z\n",
      )
      .unwrap();

      tombstone_task(Some(tmp.path()), &id, sample_deleted_at()).unwrap();

      let raw = fs::read_to_string(&path).unwrap();
      assert!(raw.contains("deleted_at: 2026-04-08T12:00:00+00:00"));
      assert!(raw.contains("title: Fix bug"));
      assert!(raw.contains("description: detail"));
      assert!(raw.contains("- bug"));
    }
  }
}
