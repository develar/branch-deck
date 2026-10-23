//! Where each CLI that is not git is looked for (`child_process::find_tool`), in one table: the order is that of its
//! installers.

use crate::child_process::{HOMEBREW_DIRS, ToolDir, ToolLookup};

const HOMEBREW_AND_LOCAL_BIN: &[ToolDir] = &[
  ToolDir::Absolute("/opt/homebrew/bin"),
  ToolDir::Absolute("/usr/local/bin"),
  ToolDir::Absolute("/home/linuxbrew/.linuxbrew/bin"),
  ToolDir::Home(".local/bin"),
];

/// `teamcity`: `PATH`, Homebrew, `~/.local/bin` (its install script), the login shell; on Windows only `PATH`
pub const TEAMCITY: ToolLookup = ToolLookup {
  name: "teamcity",
  before_path: &[],
  after_path: if cfg!(windows) { &[] } else { HOMEBREW_AND_LOCAL_BIN },
  login_shell: true,
};

/// `youtrack`: as `teamcity`, the same tap and the same kind of install script
pub const YOUTRACK: ToolLookup = ToolLookup { name: "youtrack", ..TEAMCITY };

/// `brew`: its prefix of Apple silicon, Intel and Linux, then `PATH`
pub const BREW: ToolLookup = ToolLookup {
  name: "brew",
  before_path: if cfg!(windows) {
    &[]
  } else {
    &[
      ToolDir::Absolute("/opt/homebrew/bin"),
      ToolDir::Absolute("/usr/local/bin"),
      ToolDir::Absolute("/home/linuxbrew/.linuxbrew/bin"),
    ]
  },
  after_path: &[],
  login_shell: true,
};

const CLAUDE_DIRS: &[ToolDir] = if cfg!(target_os = "macos") {
  &[
    ToolDir::Home(".local/bin"),
    ToolDir::Home(".claude/local"),
    ToolDir::Absolute("/opt/homebrew/bin"),
    ToolDir::Absolute("/usr/local/bin"),
  ]
} else {
  &[ToolDir::Home(".local/bin"), ToolDir::Home(".claude/local")]
};

/// `claude`: the native installer's `~/.local/bin` and the older `~/.claude/local`, Homebrew on macOS, `PATH`, the login
/// shell
pub const CLAUDE: ToolLookup = ToolLookup {
  name: "claude",
  before_path: CLAUDE_DIRS,
  after_path: &[],
  login_shell: true,
};

/// `gh`: Homebrew first on macOS, as for git, then `PATH`
pub const GH: ToolLookup = ToolLookup {
  name: "gh",
  before_path: if cfg!(target_os = "macos") { HOMEBREW_DIRS } else { &[] },
  after_path: &[],
  login_shell: false,
};
