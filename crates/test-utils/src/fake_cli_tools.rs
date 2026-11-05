//! Fakes of the `teamcity` and `youtrack` CLIs and of `brew` in a directory of their own (`bin`, `home`), for the
//! tests of `cli-tools` and the e2e server: no test runs the real tools of the machine. Each fake appends its argv to
//! the log and answers from state files that a test sets (the version, the `auth status --json`, the bundled skill);
//! `brew install` links a tool from the fake cellar into `bin` as Homebrew does, `brew upgrade` makes its latest
//! version the installed one.

use crate::fake_executable::{fake_executable, shell_path};
use std::path::{Path, PathBuf};

/// `teamcity auth status --json` signed in to no server: its exit code is 1
pub const TEAMCITY_SIGNED_OUT: &str = r#"[{"server":"","auth_method":"","status":"error","error":"not logged in to any TeamCity server"}]"#;
/// `youtrack auth status --json` without an instance
pub const YOUTRACK_SIGNED_OUT: &str = r#"{"url":"","status":"not configured"}"#;

pub struct FakeCliTools {
  root: PathBuf,
}

impl FakeCliTools {
  /// The fakes in `root`, nothing installed yet
  pub fn new(root: &Path) -> Self {
    let fakes = Self { root: root.to_path_buf() };
    for dir in [fakes.bin(), fakes.home(), fakes.state()] {
      std::fs::create_dir_all(dir).unwrap();
    }
    fakes.set("teamcity.auth", TEAMCITY_SIGNED_OUT);
    fakes.set("youtrack.auth", YOUTRACK_SIGNED_OUT);
    for name in ["teamcity", "youtrack"] {
      let cellar = fakes.cellar(name);
      std::fs::create_dir_all(&cellar).unwrap();
      fake_executable(&cellar, name, &fakes.tool_body(name));
    }
    fakes
  }

  /// The `PATH` of the tools
  pub fn bin(&self) -> PathBuf {
    self.root.join("bin")
  }

  /// The home of the tools: the skills of the agents
  pub fn home(&self) -> PathBuf {
    self.root.join("home")
  }

  fn state(&self) -> PathBuf {
    self.root.join("state")
  }

  /// Where `brew` keeps a tool, a `Cellar` directory as Homebrew's
  fn cellar(&self, name: &str) -> PathBuf {
    self.root.join("Cellar").join(name).join("bin")
  }

  /// Writes the state file `name` (e.g. `teamcity.auth`, `teamcity.skill`)
  pub fn set(&self, name: &str, text: &str) {
    std::fs::write(self.state().join(name), text).unwrap();
  }

  /// `name` installed by Homebrew at `version`
  pub fn install(&self, name: &str, version: &str) {
    self.set(&format!("{name}.version"), version);
    self.link(name);
  }

  /// `name` at `version` from its install script: in `bin` itself, not linked from the cellar
  pub fn install_unmanaged(&self, name: &str, version: &str) {
    self.set(&format!("{name}.version"), version);
    fake_executable(&self.bin(), name, &self.tool_body(name));
  }

  /// The release `brew upgrade`, `teamcity update` and `youtrack update` install
  pub fn set_latest(&self, name: &str, version: &str) {
    self.set(&format!("{name}.latest"), version);
  }

  /// A `brew` in `bin`
  pub fn add_brew(&self) {
    let state = shell_path(&self.state());
    let log = shell_path(&self.root.join("log"));
    let bin = shell_path(&self.bin());
    let cellar = shell_path(&self.root.join("Cellar"));
    let body = format!(
      r#"echo "brew $*" >> {log}
if [ -e {state}/brew.fail ]; then cat {state}/brew.fail >&2; exit 1; fi
if [ -e {state}/brew.hang ]; then echo "==> Fetching $2"; sleep 60; fi
name="${{2##*/}}"
case "$1" in
  install|upgrade)
    echo "==> Fetching $2"
    echo "==> Installing $name"
    if [ "$1" = upgrade ] && [ -e {state}/$name.latest ]; then cp {state}/$name.latest {state}/$name.version; fi
    if [ "$1" = install ] && [ -e {state}/$name.latest ]; then cp {state}/$name.latest {state}/$name.version; fi
    if [ ! -e {state}/$name.version ]; then echo 1.0.0 > {state}/$name.version; fi
    ln -sf {cellar}/$name/bin/$name {bin}/$name
    echo "==> Done";;
esac"#
    );
    fake_executable(&self.bin(), "brew", &body);
  }

  /// The argv of every run of a fake, in order: `teamcity --version`, `brew install jetbrains/utils/youtrack`
  pub fn log(&self) -> Vec<String> {
    std::fs::read_to_string(self.root.join("log")).map_or_else(|_| Vec::new(), |text| text.lines().map(str::to_string).collect())
  }

  fn link(&self, name: &str) {
    // no links on Windows: a copy of the fake
    #[cfg(not(unix))]
    fake_executable(&self.bin(), name, &self.tool_body(name));
    #[cfg(unix)]
    {
      let link = self.bin().join(name);
      let _ = std::fs::remove_file(&link);
      std::os::unix::fs::symlink(self.cellar(name).join(name), &link).unwrap();
    }
  }

  fn tool_body(&self, name: &str) -> String {
    let state = shell_path(&self.state());
    let log = shell_path(&self.root.join("log"));
    let skill = shell_path(&self.home().join(format!(".claude/skills/{name}-cli")));
    // the skill `<name>-cli` of the agents: the version of `<name>.skill` in this release, 1.0.0 without one
    let common = format!(
      r#"echo "{name} $*" >> {log}
bundled=$(cat {state}/{name}.skill 2>/dev/null || echo 1.0.0)
case "$1 $2" in
  "--version "*) echo "{name} version $(cat {state}/{name}.version)"; exit 0;;
  "skill list") echo "{name}-cli         $bundled  Use when working with {name}"; exit 0;;
  "skill install"|"skill update") mkdir -p {skill}; printf -- '---\nname: {name}-cli\nversion: %s\n---\n' "$bundled" > {skill}/SKILL.md; exit 0;;
  "skill remove") rm -rf {skill}; exit 0;;
esac"#
    );
    let own = match name {
      "teamcity" => format!(
        r#"case "$1 $2" in
  "update "*) if [ -e {state}/teamcity.latest ]; then cp {state}/teamcity.latest {state}/teamcity.version; fi;;
  "auth status") cat {state}/teamcity.auth; grep -q '"authenticated"' {state}/teamcity.auth || exit 1;;
  "auth login")
    # as the CLI with --web --json: the authorize event, then the token it keeps and the login event; an error is its
    # JSON envelope on stderr. A server without the browser sign-in fails before the browser
    if [ -e {state}/teamcity.no-web ]; then
      printf '{{\n  "error": {{\n    "code": "validation_error",\n    "message": "browser sign-in is not available on %s",\n    "suggestion": "Requires TeamCity 2026.1+ with PKCE enabled; use --token instead"\n  }}\n}}\n' "$4" >&2
      exit 1
    fi
    printf '{{"event":"authorize","url":"%s/oauth2/authorize?client_id=teamcity-cli&code_challenge=x","browser_opened":true}}\n' "$4"
    if [ -e {state}/teamcity.denied ]; then
      printf 'Browser auth failed\n{{\n  "error": {{\n    "code": "validation_error",\n    "message": "browser sign-in failed: access_denied"\n  }}\n}}\n' >&2
      exit 1
    fi
    # an end without the login event, as a cancel
    if [ -e {state}/teamcity.canceled ]; then exit 0; fi
    printf '[{{"server":"%s","status":"authenticated","user":{{"username":"jane"}},"token_expiry":"2026-12-01T00:00:00Z"}}]' "$4" > {state}/teamcity.auth
    printf '{{"event":"login","server":"%s","auth_method":"token","user":{{"id":1,"username":"jane","name":"Jane Doe"}},"token_storage":"keyring","token_expiry":"2026-12-01T00:00:00Z"}}\n' "$4";;
esac"#
      ),
      _ => format!(
        r#"case "$1 $2" in
  "update --yes")
    # as the CLI with --json: the lines of the upgrade on stderr, then its status on stdout; an error is its JSON
    # envelope on stderr
    echo "Running the install script" >&2
    if [ -e {state}/youtrack.update-fail ]; then
      printf '{{\n  "error": {{\n    "code": "internal_error",\n    "message": "%s"\n  }}\n}}\n' "$(cat {state}/youtrack.update-fail)" >&2
      exit 1
    fi
    before=$(cat {state}/youtrack.version)
    if [ -e {state}/youtrack.latest ]; then cp {state}/youtrack.latest {state}/youtrack.version; fi
    after=$(cat {state}/youtrack.version)
    upgraded=$([ "$before" = "$after" ] && echo false || echo true)
    printf '{{"currentVersion":"%s","latestVersion":"%s","updateAvailable":%s,"upgraded":%s}}\n' "$before" "$after" "$upgraded" "$upgraded";;
  "auth status") cat {state}/youtrack.auth;;
  "auth login")
    # as the CLI: the token from stdin, checked and kept; its instance the active one, beside the others
    read -r token; printf '%s' "$token" > {state}/youtrack.token
    host=$(echo "$4" | sed -e 's|^[a-z]*://||' -e 's|[:/.].*||')
    printf '[%s{{"alias":"%s","url":"%s","login":"jane","status":"authenticated","active":true}}]' "$(cat {state}/youtrack.others 2>/dev/null)" "$host" "$4" > {state}/youtrack.auth
    echo '{{"status":"authenticated"}}';;
  "instance use") echo "now $3";;
esac"#
      ),
    };
    format!("{common}\n{own}")
  }
}
