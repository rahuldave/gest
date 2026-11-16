//! A configured `database.url` must never crash gest. A value that is not a
//! URL gest can connect to is a configuration error (EX_CONFIG, 78), and a
//! remote database that cannot be reached is an ordinary error.

use std::net::TcpListener;

use crate::support::helpers::GestCmd;

/// Run `gest task list` with `GEST_DATABASE__URL` set to `url`.
fn task_list_with_url(url: &str) -> std::process::Output {
  let g = GestCmd::new_uninit();
  g.cmd()
    .env("GEST_DATABASE__URL", url)
    .args(["task", "list"])
    .output()
    .expect("task list failed to run")
}

fn assert_config_error(url: &str) {
  let output = task_list_with_url(url);
  let stderr = String::from_utf8_lossy(&output.stderr);

  assert_eq!(
    output.status.code(),
    Some(78),
    "database.url {url:?} should exit EX_CONFIG (78), got {:?}: {stderr}",
    output.status.code()
  );
  assert!(stderr.contains("database.url"), "the error names the key: {stderr}");
  assert!(!stderr.contains("panicked"), "gest must not panic: {stderr}");
}

#[test]
fn it_exits_78_when_database_url_is_not_a_url() {
  assert_config_error("not a url");
}

#[test]
fn it_exits_78_when_database_url_has_an_unsupported_scheme() {
  assert_config_error("postgres://db.example.com:5432");
}

#[test]
fn it_exits_78_when_database_url_has_no_host() {
  assert_config_error("libsql://");
}

#[test]
fn it_fails_without_a_panic_when_the_remote_database_is_unreachable() {
  // Bind then drop a listener, so the port is free and nothing answers on it.
  let port = {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    listener.local_addr().expect("local addr").port()
  };

  let output = task_list_with_url(&format!("http://127.0.0.1:{port}"));
  let stderr = String::from_utf8_lossy(&output.stderr);

  assert_ne!(
    output.status.code(),
    Some(0),
    "an unreachable database cannot succeed: {stderr}"
  );
  assert_ne!(output.status.code(), Some(101), "gest must not panic: {stderr}");
  assert!(!stderr.contains("panicked"), "gest must not panic: {stderr}");
}
