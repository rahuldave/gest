//! Integration tests for `gest project unarchive`.

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
  let extra_dir = g.temp_dir_path().join("env-unarchive");
  g.init_extra_project(&extra_dir);

  let id = project_id_by_root_suffix(&g, "env-unarchive");
  g.cmd().args(["project", "archive", &id, "--yes"]).assert().success();

  let output = g
    .cmd()
    .args(["project", "unarchive", &id, "--json"])
    .output()
    .expect("project unarchive --json failed");

  assert!(
    output.status.success(),
    "project unarchive --json exited non-zero: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");

  assert_eq!(parsed["id"].as_str(), Some(id.as_str()), "id field missing or wrong");
  assert!(parsed["root"].is_string(), "root field missing: {stdout}");
  assert!(
    parsed["archived_at"].is_null(),
    "archived_at should be null after unarchive: {stdout}"
  );
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
}

#[test]
fn it_emits_short_id_in_quiet_mode() {
  let g = GestCmd::new();
  let extra_dir = g.temp_dir_path().join("quiet-unarchive");
  g.init_extra_project(&extra_dir);

  let id = project_id_by_root_suffix(&g, "quiet-unarchive");
  g.cmd().args(["project", "archive", &id, "--yes"]).assert().success();

  let output = g
    .cmd()
    .args(["project", "unarchive", &id, "--quiet"])
    .output()
    .expect("project unarchive --quiet failed");

  assert!(
    output.status.success(),
    "project unarchive --quiet exited non-zero: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  let trimmed = stdout.trim();

  assert!(
    id.starts_with(trimmed),
    "quiet mode should emit a short prefix of the id, got: {trimmed}"
  );
}

#[test]
fn it_emits_workspace_hint_in_human_mode() {
  let g = GestCmd::new();
  let extra_dir = g.temp_dir_path().join("hint-unarchive");
  g.init_extra_project(&extra_dir);

  let id = project_id_by_root_suffix(&g, "hint-unarchive");
  g.cmd().args(["project", "archive", &id, "--yes"]).assert().success();

  let output = g
    .cmd()
    .args(["project", "unarchive", &id])
    .output()
    .expect("project unarchive failed");

  assert!(
    output.status.success(),
    "project unarchive exited non-zero: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  let plain = strip_ansi(&stdout);

  assert!(
    plain.contains("unarchived project"),
    "should show success message, got: {plain}"
  );
  assert!(
    plain.contains("Workspace paths are not automatically restored"),
    "should print workspace reattach hint, got: {plain}"
  );
}
