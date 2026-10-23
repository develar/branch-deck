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

  /// The release `brew upgrade` and `teamcity update` install
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
    if cfg!(windows) {
      fake_executable(&self.bin(), name, &self.tool_body(name));
      return;
    }
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
  "update "*) if [ -e {state}/{name}.latest ]; then cp {state}/{name}.latest {state}/{name}.version; fi; exit 0;;
  "skill list") echo "{name}-cli         $bundled  Use when working with {name}"; exit 0;;
  "skill install"|"skill update") mkdir -p {skill}; printf -- '---\nname: {name}-cli\nversion: %s\n---\n' "$bundled" > {skill}/SKILL.md; exit 0;;
  "skill remove") rm -rf {skill}; exit 0;;
esac"#
    );
    let own = match name {
      "teamcity" => format!(
        r#"case "$1 $2" in
  "auth status") cat {state}/teamcity.auth; grep -q '"authenticated"' {state}/teamcity.auth || exit 1;;
  "auth login")
    # as the CLI: the authorize page, then the token it keeps; a server without the browser sign-in asks to paste one
    if [ -e {state}/teamcity.paste ]; then printf 'Paste your access token: '; read -r token; exit 1; fi
    echo "Opening $4/oauth2/authorize?client_id=teamcity-cli&code_challenge=x in your browser"
    if [ -e {state}/teamcity.denied ]; then echo "Error: the sign-in was denied"; exit 1; fi
    printf '[{{"server":"%s","status":"authenticated","user":{{"username":"jane"}},"token_expiry":"2026-12-01T00:00:00Z"}}]' "$4" > {state}/teamcity.auth
    echo "Logged in to $4 as jane";;
esac"#
      ),
      _ => format!(
        r#"case "$1 $2" in
  "auth status") cat {state}/youtrack.auth;;
esac"#
      ),
    };
    format!("{common}\n{own}")
  }
}
