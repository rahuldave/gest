use assert_cmd::Command;
use tempfile::TempDir;

use crate::support::helpers::GestCmd;

fn isolated_cmd(dir: &TempDir) -> Command {
  let path = dir.path();
  let mut cmd = Command::cargo_bin("gest").expect("gest binary not found");
  cmd
    .current_dir(path)
    .env("GEST_CONFIG", path.join("gest.toml"))
    .env("XDG_CONFIG_HOME", path.join(".xdg-config"))
    .env("XDG_DATA_HOME", path.join(".xdg-data"))
    .env("NO_COLOR", "1")
    .env_remove("GEST_DATABASE__URL")
    .env_remove("GEST_STORAGE__DATA_DIR");
  cmd
}

#[test]
fn it_defaults_initialized_local_projects_to_the_gest_sqlite_cache() {
  let dir = TempDir::new().expect("temp dir");

  isolated_cmd(&dir).args(["init", "--local"]).assert().success();
  assert!(dir.path().join(".gest/project.yaml").is_file());

  isolated_cmd(&dir)
    .args(["task", "create", "Project-local storage"])
    .assert()
    .success();

  assert!(
    dir.path().join(".gest/gest.db").is_file(),
    "initialized local projects should use .gest/gest.db by default"
  );
  assert!(
    !dir.path().join(".xdg-data/gest/gest.db").exists(),
    "default local project commands should not keep using the global XDG data store"
  );
}

#[test]
fn it_recreates_the_project_local_cache_from_synced_files() {
  let dir = TempDir::new().expect("temp dir");

  isolated_cmd(&dir).args(["init", "--local"]).assert().success();
  isolated_cmd(&dir)
    .args(["task", "create", "Fresh checkout task"])
    .assert()
    .success();

  let gest_dir = dir.path().join(".gest");
  std::fs::remove_file(gest_dir.join("gest.db")).expect("remove local sqlite db");
  let _ = std::fs::remove_file(gest_dir.join("gest.db-wal"));
  let _ = std::fs::remove_file(gest_dir.join("gest.db-shm"));

  let output = isolated_cmd(&dir)
    .args(["task", "list", "--all"])
    .output()
    .expect("task list failed to run");
  assert!(
    output.status.success(),
    "fresh checkout import failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(String::from_utf8_lossy(&output.stdout).contains("Fresh checkout task"));

  assert!(
    gest_dir.join("gest.db").is_file(),
    "fresh checkout commands should recreate .gest/gest.db"
  );
}

#[test]
fn it_preserves_explicit_storage_data_dir_overrides() {
  let g = GestCmd::new();

  g.create_task("Explicit data dir");

  assert!(g.db_path().is_file());
  assert!(
    !g.temp_dir_path().join(".gest/gest.db").exists(),
    "GEST_STORAGE__DATA_DIR should opt out of project-local sqlite"
  );
}
