use getset::Getters;
use serde::{Deserialize, Serialize};

/// Database connection settings from the `[database]` configuration table.
///
/// Connection details can be provided either as a complete URL or as individual
/// components (scheme, host, port, username, password). When both a URL and
/// individual components are present, the explicit URL takes precedence.
#[derive(Clone, Debug, Default, Deserialize, Getters, PartialEq, Serialize)]
#[serde(default)]
pub struct Settings {
  /// Bearer or API token for database authentication.
  #[get = "pub"]
  auth_token: Option<String>,
  /// Database server hostname or IP address.
  #[get = "pub"]
  host: Option<String>,
  /// Password for username/password authentication.
  #[get = "pub"]
  password: Option<String>,
  /// Port number the database server listens on.
  #[get = "pub"]
  port: Option<u16>,
  /// Connection scheme: `libsql`, `https` or `http`. Defaults to `libsql` when building a URL
  /// from components.
  #[get = "pub"]
  scheme: Option<String>,
  /// Explicit connection URL. When set, component fields are ignored by [`Settings::url`].
  url: Option<String>,
  /// Username for database authentication.
  #[get = "pub"]
  username: Option<String>,
}

impl Settings {
  /// Returns the database connection URL.
  ///
  /// If an explicit `url` was configured, it is returned as-is. Otherwise a URL is assembled
  /// from the individual component fields in the form `scheme://[user[:pass]@]host[:port]`.
  ///
  /// # Returns
  ///
  /// The connection URL. Returns `None` when no `url` or `host` is configured.
  pub fn url(&self) -> Option<String> {
    if let Some(url) = &self.url {
      log::debug!("database.url is set to: {url}");
      log::trace!("using database.url");
      return Some(url.clone());
    }

    let host = self.host.as_deref()?;
    let scheme = self.scheme.as_deref().unwrap_or("libsql");
    let mut url = format!("{scheme}://");

    if let Some(username) = &self.username {
      url.push_str(username);
      if let Some(password) = &self.password {
        url.push(':');
        url.push_str(password);
      }
      url.push('@');
    }

    url.push_str(host);

    if let Some(port) = self.port {
      url.push(':');
      url.push_str(&port.to_string());
    }

    log::debug!("database url resolves to {url}");
    log::trace!("using {url}");
    Some(url)
  }
}

/// The URL schemes the libsql remote client can connect with. It sends `libsql://` over HTTPS.
const REMOTE_SCHEMES: [&str; 3] = ["libsql", "https", "http"];

/// Check that `url` is a remote database URL that libsql can connect to.
///
/// The URL must be `libsql://`, `https://` or `http://`, followed by a host, an optional
/// `user[:pass]@` before it and an optional numeric port after it.
///
/// # Arguments
///
/// * `url` - the configured or assembled database URL
///
/// # Returns
///
/// `Ok(())` for a usable URL, or the reason it is not usable. The reason never repeats the URL,
/// because the URL can hold a password.
pub fn validate_remote_url(url: &str) -> Result<(), String> {
  let supported = "it must start with libsql://, https:// or http:// and name a host";
  if url.chars().any(char::is_whitespace) {
    return Err(format!("it contains a space; {supported}"));
  }
  let Some((scheme, rest)) = url.split_once("://") else {
    return Err(format!("it has no scheme; {supported}"));
  };
  if !REMOTE_SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) {
    return Err(format!("the scheme {scheme:?} is not supported; {supported}"));
  }
  let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
  let host_port = authority.rsplit_once('@').map_or(authority, |(_, host_port)| host_port);
  let (host, port) = if let Some(bracketed) = host_port.strip_prefix('[') {
    let Some((host, after)) = bracketed.split_once(']') else {
      return Err(format!("its IPv6 host has no closing ]; {supported}"));
    };
    match after {
      "" => (host, None),
      _ => match after.strip_prefix(':') {
        Some(port) => (host, Some(port)),
        None => return Err(format!("text follows its IPv6 host; {supported}")),
      },
    }
  } else {
    match host_port.split_once(':') {
      Some((host, port)) => (host, Some(port)),
      None => (host_port, None),
    }
  };
  if host.is_empty() {
    return Err(format!("it names no host; {supported}"));
  }
  if let Some(port) = port
    && port.parse::<u16>().is_err()
  {
    return Err(format!("its port is not a number from 0 to 65535; {supported}"));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  mod url {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_full_url_from_all_components() {
      let settings = Settings {
        scheme: Some("libsql".into()),
        host: Some("db.example.com".into()),
        port: Some(8080),
        username: Some("user".into()),
        password: Some("pass".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://user:pass@db.example.com:8080".into()));
    }

    #[test]
    fn it_builds_url_from_host_with_default_scheme() {
      let settings = Settings {
        host: Some("localhost".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://localhost".into()));
    }

    #[test]
    fn it_builds_url_with_custom_scheme() {
      let settings = Settings {
        scheme: Some("libsql".into()),
        host: Some("db.example.com".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://db.example.com".into()));
    }

    #[test]
    fn it_ignores_password_without_username() {
      let settings = Settings {
        host: Some("localhost".into()),
        password: Some("secret".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://localhost".into()));
    }

    #[test]
    fn it_includes_port() {
      let settings = Settings {
        host: Some("localhost".into()),
        port: Some(5432),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://localhost:5432".into()));
    }

    #[test]
    fn it_includes_username() {
      let settings = Settings {
        host: Some("localhost".into()),
        username: Some("admin".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://admin@localhost".into()));
    }

    #[test]
    fn it_includes_username_and_password() {
      let settings = Settings {
        host: Some("localhost".into()),
        username: Some("admin".into()),
        password: Some("secret".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://admin:secret@localhost".into()));
    }

    #[test]
    fn it_returns_explicit_url_as_is() {
      let settings = Settings {
        url: Some("libsql://my-db.turso.io".into()),
        host: Some("ignored.example.com".into()),
        ..Default::default()
      };

      assert_eq!(settings.url(), Some("libsql://my-db.turso.io".into()));
    }

    #[test]
    fn it_returns_none_when_no_url_or_host() {
      let settings = Settings::default();

      assert_eq!(settings.url(), None);
    }
  }

  mod validate_remote_url {
    use super::*;

    #[test]
    fn it_accepts_the_schemes_libsql_can_connect_with() {
      for url in [
        "libsql://my-project.turso.io",
        "https://db.example.com",
        "http://127.0.0.1:8080",
        "HTTPS://db.example.com/path?x=1",
        "libsql://user:pass@db.example.com:443",
        "http://[::1]:8080",
        "http://[::1]",
      ] {
        assert_eq!(validate_remote_url(url), Ok(()), "{url}");
      }
    }

    #[test]
    fn it_rejects_urls_libsql_cannot_use() {
      for url in [
        "not a url",
        "db.example.com",
        "postgres://db.example.com",
        "sqlite://localhost",
        "libsql://",
        "https://user:pass@",
        "http://host:port",
        "http://host:70000",
        "http://[::1",
        "http://[::1]x",
        "",
      ] {
        assert!(validate_remote_url(url).is_err(), "{url:?} should be rejected");
      }
    }

    #[test]
    fn it_does_not_repeat_the_url_in_the_reason() {
      let reason = validate_remote_url("postgres://admin:hunter2@db.example.com").unwrap_err();

      assert!(!reason.contains("hunter2"), "{reason}");
    }

    #[test]
    fn it_accepts_the_url_built_from_parts_with_the_default_scheme() {
      let settings = Settings {
        host: Some("db.example.com".into()),
        port: Some(8080),
        ..Default::default()
      };

      assert_eq!(validate_remote_url(&settings.url().unwrap()), Ok(()));
    }
  }
}
