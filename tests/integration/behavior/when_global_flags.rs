//! Tests for global CLI flags that apply across every command: `--help`, `--version`,
//! and error behavior for unknown flags/subcommands.

use crate::support::helpers::GestCmd;

#[test]
fn it_prints_help_with_long_flag() {
  let g = GestCmd::new_uninit();

  let output = g.cmd().args(["--help"]).output().expect("--help failed to run");

  assert!(output.status.success(), "--help should exit zero");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("Usage:"),
    "help output should contain Usage, got: {stdout}"
  );

  // Each top-level subcommand should also expose --help.
  for sub in &[
    "artifact",
    "config",
    "generate",
    "init",
    "iteration",
    "project",
    "search",
    "tag",
    "task",
    "undo",
  ] {
    let out = g
      .cmd()
      .args([sub, "--help"])
      .output()
      .unwrap_or_else(|_| panic!("{sub} --help failed to run"));
    assert!(out.status.success(), "{sub} --help should exit zero");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Usage:"), "{sub} help should contain Usage, got: {text}");
  }
}

#[test]
fn it_prints_version_with_long_flag() {
  let g = GestCmd::new_uninit();

  let output = g.cmd().args(["--version"]).output().expect("--version failed to run");

  assert!(output.status.success(), "--version should exit zero");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains(env!("CARGO_PKG_VERSION")),
    "--version output should contain crate version, got: {stdout}"
  );
}

#[test]
fn it_prints_version_with_short_flag() {
  let g = GestCmd::new_uninit();

  let output = g.cmd().args(["-V"]).output().expect("-V failed to run");

  assert!(output.status.success(), "-V should exit zero");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains(env!("CARGO_PKG_VERSION")),
    "-V output should contain crate version, got: {stdout}"
  );
}

#[test]
fn it_rejects_unknown_flag() {
  let g = GestCmd::new_uninit();

  let output = g
    .cmd()
    .args(["--definitely-not-a-flag"])
    .output()
    .expect("unknown flag failed to run");

  assert!(!output.status.success(), "unknown flag should exit non-zero");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.to_lowercase().contains("unexpected") || stderr.to_lowercase().contains("unknown"),
    "stderr should mention unknown/unexpected argument, got: {stderr}"
  );
}

#[test]
fn it_advertises_no_pager_in_top_level_help() {
  let g = GestCmd::new_uninit();

  let output = g.cmd().args(["--help"]).output().expect("--help failed to run");

  assert!(output.status.success(), "--help should exit zero");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("--no-pager"),
    "top-level --help should advertise --no-pager, got: {stdout}"
  );
}

#[test]
fn it_accepts_no_pager_before_subcommand() {
  let g = GestCmd::new_uninit();

  let output = g
    .cmd()
    .args(["--no-pager", "--version"])
    .output()
    .expect("--no-pager --version failed to run");

  assert!(
    output.status.success(),
    "--no-pager before --version should exit zero, stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn it_accepts_no_pager_after_subcommand() {
  let g = GestCmd::new_uninit();

  let output = g
    .cmd()
    .args(["version", "--no-pager"])
    .output()
    .expect("version --no-pager failed to run");

  assert!(
    output.status.success(),
    "version --no-pager should exit zero, stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn it_rejects_unknown_subcommand() {
  let g = GestCmd::new_uninit();

  let output = g
    .cmd()
    .args(["definitely-not-a-subcommand"])
    .output()
    .expect("unknown subcommand failed to run");

  assert!(!output.status.success(), "unknown subcommand should exit non-zero");
}

#[test]
fn it_runs_with_typed_and_table_named_gest_variables_set() {
  let g = GestCmd::new();

  for (key, value) in [
    ("GEST_STORAGE__SYNC", "false"),
    ("GEST_STORAGE__SYNC", "yes"),
    ("GEST_SERVE__PORT", "8123"),
    ("GEST_PAGER", "less"),
  ] {
    let output = g
      .cmd()
      .env(key, value)
      .args(["task", "list"])
      .output()
      .expect("failed to run gest task list");

    assert!(
      output.status.success(),
      "{key}={value}: gest task list failed: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  }
}

#[test]
fn it_warns_once_about_an_unknown_palette_key_from_the_environment() {
  let g = GestCmd::new();

  let output = g
    .cmd()
    .env("GEST_COLORS__PALETTE__BOGUS", "red")
    .args(["task", "list"])
    .output()
    .expect("failed to run gest task list");

  assert!(output.status.success());
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert_eq!(stderr.matches("unknown palette key").count(), 1, "stderr: {stderr}");
}
