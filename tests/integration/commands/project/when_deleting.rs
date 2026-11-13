//! Integration tests for `gest project delete`.

use crate::support::helpers::{GestCmd, strip_ansi};

/// Extract the full project ID from `project list --json` output whose root
/// path ends with the given `suffix`.
fn project_id_by_root_suffix(g: &GestCmd, suffix: &str) -> String {
  let output = g
    .cmd()
    .args(["project", "list", "--all", "--json"])
    .output()
    .expect("project list --json failed");
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
  let arr = parsed.as_array().expect("JSON array");
  for entry in arr {
    if let Some(r) = entry["root"].as_str()
      && r.ends_with(suffix)
    {
      return entry["id"].as_str().unwrap().to_string();
    }
  }
  panic!("no project found with root ending in {suffix} in: {stdout}");
}

#[test]
fn it_emits_envelope_shape_in_json_mode() {
  let g = GestCmd::new();
  let project_id = project_id_by_root_suffix(&g, g.temp_dir_path().file_name().unwrap().to_str().unwrap());

  let output = g
    .cmd()
    .args(["project", "delete", &project_id, "--yes", "--json"])
    .output()
    .expect("project delete --json failed");

  assert!(
    output.status.success(),
    "project delete --json exited non-zero: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");

  assert_eq!(
    parsed["id"].as_str(),
    Some(project_id.as_str()),
    "id field missing or wrong"
  );
  assert!(parsed["root"].is_string(), "root field missing: {stdout}");
  assert!(parsed["created_at"].is_string(), "created_at field missing: {stdout}");
  assert!(parsed["updated_at"].is_string(), "updated_at field missing: {stdout}");
  assert!(
    parsed["tags"].is_array(),
    "envelope should include tags array: {stdout}"
  );
  assert!(
    parsed["relationships"].is_array(),
    "envelope should include relationships array: {stdout}"
  );
  assert!(
    parsed.get("tasks").is_none(),
    "old custom-shape field tasks should no longer appear at top level: {stdout}"
  );
  assert!(
    parsed.get("notes").is_none() || parsed["notes"].is_array(),
    "notes sidecar, if present, should be an array: {stdout}"
  );
}

#[test]
fn it_emits_short_id_in_quiet_mode() {
  let g = GestCmd::new();
  let project_id = project_id_by_root_suffix(&g, g.temp_dir_path().file_name().unwrap().to_str().unwrap());

  let output = g
    .cmd()
    .args(["project", "delete", &project_id, "--yes", "--quiet"])
    .output()
    .expect("project delete --quiet failed");

  assert!(
    output.status.success(),
    "project delete --quiet exited non-zero: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  let trimmed = stdout.trim();

  assert!(
    project_id.starts_with(trimmed),
    "quiet mode should emit a short prefix of the id, got: {trimmed}"
  );
}

#[test]
fn it_emits_human_message_by_default() {
  let g = GestCmd::new();
  let project_id = project_id_by_root_suffix(&g, g.temp_dir_path().file_name().unwrap().to_str().unwrap());

  let output = g
    .cmd()
    .args(["project", "delete", &project_id, "--yes"])
    .output()
    .expect("project delete failed");

  assert!(
    output.status.success(),
    "project delete exited non-zero: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  let plain = strip_ansi(&stdout);

  assert!(
    plain.contains("deleted project"),
    "should show success message, got: {plain}"
  );
}
