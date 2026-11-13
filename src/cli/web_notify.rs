//! Fire-and-forget IPC helper that pings a running web server to trigger a reload.
//!
//! The web server (when running) listens on a unix domain socket. CLI mutations call
//! [`notify_web_reload`] to wake any subscribed browser tabs via SSE. Absence of the
//! server is the common case and must remain silent and fast.

use std::path::Path;

/// Connect timeout for the reload notifier.
///
/// Kept intentionally small so that the absence of a running web server adds no
/// user-visible latency to CLI commands. The notifier never blocks the caller for
/// longer than this on the happy "no server" path.
#[cfg(unix)]
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);

/// Notify a running web server that project state has changed.
///
/// Resolves the same socket path as [`crate::web::reload_socket_path`] and opens a
/// short-lived `UnixStream`. No bytes are written or read — the bare connection itself
/// is the signal. Any error (file missing, refused, timeout) is treated as "no server
/// running" and silently swallowed.
///
/// On non-unix targets this function compiles to a no-op that returns `Ok(())`.
///
/// # Arguments
///
/// * `gest_dir` - the project's `.gest` directory, if it has one
/// * `data_dir` - the global data directory, used for the socket when there is no `.gest` directory
///
/// # Returns
///
/// Always `Ok(())`.
#[cfg(unix)]
pub async fn notify_web_reload(gest_dir: Option<&Path>, data_dir: &Path) -> std::io::Result<()> {
  use tokio::{net::UnixStream, time::timeout};

  let socket_path = crate::web::reload_socket_path(gest_dir, data_dir);
  match timeout(CONNECT_TIMEOUT, UnixStream::connect(&socket_path)).await {
    Ok(Ok(_stream)) => {
      // Drop the stream immediately; the bare connection is the signal.
      Ok(())
    }
    Ok(Err(_)) | Err(_) => Ok(()),
  }
}

/// Windows stub: there is no unix socket to connect to, so this is a no-op.
#[cfg(not(unix))]
pub async fn notify_web_reload(_gest_dir: Option<&Path>, _data_dir: &Path) -> std::io::Result<()> {
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  mod notify_web_reload {
    use super::*;

    #[tokio::test]
    async fn it_returns_ok_when_no_socket_is_present() {
      let tmp = tempfile::tempdir().unwrap();

      let start = std::time::Instant::now();
      let result = notify_web_reload(None, tmp.path()).await;
      let elapsed = start.elapsed();

      assert!(result.is_ok());
      assert!(
        elapsed < std::time::Duration::from_millis(500),
        "notifier should not block when server is absent (took {elapsed:?})"
      );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn it_triggers_an_accept_on_a_listening_socket() {
      use tokio::net::UnixListener;

      let tmp = tempfile::tempdir().unwrap();
      let socket_path = crate::web::reload_socket_path(None, tmp.path());
      let listener = UnixListener::bind(&socket_path).unwrap();

      let accept_task = tokio::spawn(async move { listener.accept().await });
      let result = notify_web_reload(None, tmp.path()).await;
      let accepted = tokio::time::timeout(std::time::Duration::from_secs(1), accept_task)
        .await
        .expect("accept should complete")
        .expect("join should succeed");

      assert!(result.is_ok());
      assert!(accepted.is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn it_uses_gest_dir_when_provided() {
      use tokio::net::UnixListener;

      let tmp = tempfile::tempdir().unwrap();
      let gest_dir = tmp.path().join(".gest");
      std::fs::create_dir_all(&gest_dir).unwrap();
      let socket_path = crate::web::reload_socket_path(Some(&gest_dir), tmp.path());
      let listener = UnixListener::bind(&socket_path).unwrap();

      let accept_task = tokio::spawn(async move { listener.accept().await });
      let result = notify_web_reload(Some(&gest_dir), tmp.path()).await;
      let accepted = tokio::time::timeout(std::time::Duration::from_secs(1), accept_task)
        .await
        .expect("accept should complete")
        .expect("join should succeed");

      assert!(result.is_ok());
      assert!(accepted.is_ok());
    }
  }
}
