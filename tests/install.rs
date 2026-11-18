//! scripts/install.sh against a fake release that the test makes. Stubs of uname and curl stand in for the platform
//! and the network, so the test reads no network. The tests need sh, tar, cut, sed and shasum on the PATH.

use std::{
  fs,
  os::unix::fs::PermissionsExt,
  path::{Path, PathBuf},
  process::{Command, Output},
};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TARGETS: [&str; 4] = [
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
  "x86_64-unknown-linux-musl",
  "aarch64-unknown-linux-musl",
];

const DOWNLOAD: &str = "https://github.com/rahuldave/gest/releases/download";

/// uname prints FAKE_UNAME_S or FAKE_UNAME_M. curl logs each URL and copies the file of the fake release for it.
const UNAME: &str = "#!/bin/sh\ncase \"$1\" in -s) echo \"$FAKE_UNAME_S\" ;; -m) echo \"$FAKE_UNAME_M\" ;; esac\n";
const CURL: &str = r#"#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) out="$2"; shift 2 ;;
        -*) shift ;;
        *) url="$1"; shift ;;
    esac
done
echo "$url" >> "$FAKE_RELEASE/curl.log"
case "$url" in
    https://api.github.com/repos/rahuldave/gest/releases/latest) file="$FAKE_RELEASE/latest.json" ;;
    https://github.com/rahuldave/gest/releases/download/*) file="$FAKE_RELEASE/${url#https://github.com/rahuldave/gest/releases/download/}" ;;
    *) exit 22 ;;
esac
[ -f "$file" ] || exit 22
if [ -n "$out" ]; then cp "$file" "$out"; else cat "$file"; fi
"#;

struct Fake {
  temp: TempDir,
}

impl Fake {
  /// A fake release v9.9.9 with an archive and a hash for each target. Each archive holds a gest that prints
  /// "gest 9.9.9 <target>", so a test can see which archive was installed.
  fn new() -> Fake {
    let temp = TempDir::new().expect("tempdir");
    let fake = Fake {
      temp,
    };
    let stubs = fake.path("stubs");
    fs::create_dir_all(&stubs).expect("mkdir");
    write_program(&stubs.join("uname"), UNAME);
    write_program(&stubs.join("curl"), CURL);
    fs::create_dir_all(fake.path("release/v9.9.9")).expect("mkdir");
    fs::write(
      fake.path("release/latest.json"),
      "{\n  \"url\": \"x\",\n  \"tag_name\": \"v9.9.9\",\n  \"name\": \"v9.9.9\"\n}\n",
    )
    .expect("write");
    for target in TARGETS {
      let pack = fake.path(&format!("pack/{target}"));
      fs::create_dir_all(&pack).expect("mkdir");
      write_program(
        &pack.join("gest"),
        &format!("#!/bin/sh\necho \"gest 9.9.9 {target}\"\n"),
      );
      let archive = fake.path("release/v9.9.9").join(format!("gest-{target}.tar.gz"));
      let status = Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .args(["-czf", archive.to_str().expect("path"), "-C"])
        .arg(&pack)
        .arg("gest")
        .status()
        .expect("tar");
      assert!(status.success());
      fake.write_hash(target, &hex(&fs::read(&archive).expect("read")));
    }
    fake
  }

  fn path(&self, name: &str) -> PathBuf {
    self.temp.path().join(name)
  }

  fn write_hash(&self, target: &str, hash: &str) {
    fs::write(
      self.path(&format!("release/v9.9.9/gest-{target}.tar.gz.sha256")),
      format!("{hash}  gest-{target}.tar.gz\n"),
    )
    .expect("write");
  }

  /// Run install.sh with the stubs first on the PATH, on the platform SYSTEM and MACHINE.
  fn install(&self, system: &str, machine: &str, args: &[&str]) -> Output {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/install.sh");
    let path = format!(
      "{}:{}",
      self.path("stubs").display(),
      std::env::var("PATH").unwrap_or_default()
    );
    Command::new("sh")
      .arg(script)
      .args(args)
      .env("PATH", path)
      .env("HOME", self.path("home"))
      .env("FAKE_UNAME_S", system)
      .env("FAKE_UNAME_M", machine)
      .env("FAKE_RELEASE", self.path("release"))
      .output()
      .expect("run install.sh")
  }

  fn urls(&self) -> Vec<String> {
    fs::read_to_string(self.path("release/curl.log"))
      .unwrap_or_default()
      .lines()
      .map(str::to_owned)
      .collect()
  }

  fn dir(&self) -> String {
    self.path("bin").to_str().expect("path").to_owned()
  }
}

fn write_program(path: &Path, text: &str) {
  fs::write(path, text).expect("write");
  fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn hex(bytes: &[u8]) -> String {
  Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn text(output: &Output) -> String {
  format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  )
}

#[test]
fn each_platform_gets_its_target() {
  let platforms = [
    ("Darwin", "arm64", "aarch64-apple-darwin"),
    ("Darwin", "x86_64", "x86_64-apple-darwin"),
    ("Linux", "x86_64", "x86_64-unknown-linux-musl"),
    ("Linux", "amd64", "x86_64-unknown-linux-musl"),
    ("Linux", "aarch64", "aarch64-unknown-linux-musl"),
    ("Linux", "arm64", "aarch64-unknown-linux-musl"),
  ];
  for (system, machine, target) in platforms {
    let fake = Fake::new();
    let output = fake.install(system, machine, &["--tag", "v9.9.9", "--dir", &fake.dir()]);
    assert!(output.status.success(), "{system} {machine}: {}", text(&output));
    assert_eq!(
      String::from_utf8_lossy(&output.stdout),
      format!("gest 9.9.9 {target}\n")
    );
    let asset = format!("{DOWNLOAD}/v9.9.9/gest-{target}.tar.gz");
    assert_eq!(fake.urls(), [asset.clone(), format!("{asset}.sha256")]);
  }
}

#[test]
fn no_tag_installs_the_latest_release() {
  let fake = Fake::new();
  let output = fake.install("Darwin", "arm64", &["--dir", &fake.dir()]);
  assert!(output.status.success(), "{}", text(&output));
  assert_eq!(
    fake.urls()[0],
    "https://api.github.com/repos/rahuldave/gest/releases/latest"
  );
  assert_eq!(
    fake.urls()[1],
    format!("{DOWNLOAD}/v9.9.9/gest-aarch64-apple-darwin.tar.gz")
  );
}

#[test]
fn the_default_dir_is_local_bin() {
  let fake = Fake::new();
  let output = fake.install("Linux", "x86_64", &["--tag", "v9.9.9"]);
  assert!(output.status.success(), "{}", text(&output));
  assert!(fake.path("home/.local/bin/gest").is_file());
}

#[test]
fn a_wrong_hash_fails_and_installs_nothing() {
  let fake = Fake::new();
  fake.write_hash("aarch64-apple-darwin", &"0".repeat(64));
  let output = fake.install("Darwin", "arm64", &["--tag", "v9.9.9", "--dir", &fake.dir()]);
  assert!(!output.status.success());
  assert!(text(&output).contains("does not match"), "{}", text(&output));
  assert!(!fake.path("bin").exists());
}

#[test]
fn an_empty_hash_fails() {
  let fake = Fake::new();
  fake.write_hash("aarch64-apple-darwin", "");
  let output = fake.install("Darwin", "arm64", &["--tag", "v9.9.9", "--dir", &fake.dir()]);
  assert!(!output.status.success());
  assert!(!fake.path("bin").exists());
}

#[test]
fn a_missing_release_fails() {
  let fake = Fake::new();
  let output = fake.install("Darwin", "arm64", &["--tag", "v0.0.1", "--dir", &fake.dir()]);
  assert!(!output.status.success());
  assert!(!fake.path("bin").exists());
}

#[test]
fn an_unknown_platform_fails_before_any_download() {
  for (system, machine) in [("FreeBSD", "amd64"), ("Linux", "riscv64"), ("Darwin", "ppc")] {
    let fake = Fake::new();
    let output = fake.install(system, machine, &["--tag", "v9.9.9", "--dir", &fake.dir()]);
    assert!(!output.status.success(), "{system} {machine}");
    assert!(text(&output).contains("no release for"), "{}", text(&output));
    assert!(fake.urls().is_empty());
  }
}

#[test]
fn a_second_install_replaces_the_binary_with_a_new_file() {
  use std::os::unix::fs::MetadataExt;
  let fake = Fake::new();
  let dir = fake.dir();
  let args = ["--tag", "v9.9.9", "--dir", &dir];
  assert!(fake.install("Darwin", "arm64", &args).status.success());
  let first = fs::metadata(fake.path("bin/gest")).expect("gest").ino();
  let output = fake.install("Darwin", "arm64", &args);
  assert!(output.status.success(), "{}", text(&output));
  // A rename gives a new inode. A copy over the old file keeps the inode, and fails on a running binary.
  assert_ne!(fs::metadata(fake.path("bin/gest")).expect("gest").ino(), first);
  let names: Vec<_> = fs::read_dir(fake.path("bin"))
    .expect("dir")
    .map(|entry| entry.expect("entry").file_name())
    .collect();
  assert_eq!(names, ["gest"]);
}
