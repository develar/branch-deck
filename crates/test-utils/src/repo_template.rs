use crate::git_test_utils::disable_background_maintenance;
use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

/// A builder for creating test repository templates
pub struct RepoTemplate {
  #[expect(dead_code)]
  name: String,
  branch_prefix: Option<String>,
  /// Other git config, e.g. `branchdeck.stacks`
  config: Vec<(String, String)>,
  commits: Vec<CommitSpec>,
}

struct CommitSpec {
  message: String,
  files: Vec<(String, String)>,
  timestamp: Option<i64>,
}

impl RepoTemplate {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      branch_prefix: None,
      config: Vec::new(),
      commits: Vec::new(),
    }
  }

  #[must_use]
  pub fn config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.config.push((key.into(), value.into()));
    self
  }

  #[must_use]
  pub fn branch_prefix(mut self, prefix: impl Into<String>) -> Self {
    self.branch_prefix = Some(prefix.into());
    self
  }

  #[must_use]
  pub fn commit(self, message: impl Into<String>, files: &[(&str, &str)]) -> Self {
    self.commit_with_timestamp(message, files, None)
  }

  #[must_use]
  pub fn commit_with_timestamp(mut self, message: impl Into<String>, files: &[(&str, &str)], timestamp: Option<i64>) -> Self {
    let files = files.iter().map(|(path, content)| (path.to_string(), content.to_string())).collect();

    self.commits.push(CommitSpec {
      message: message.into(),
      files,
      timestamp,
    });
    self
  }

  /// Build the repository at the specified path
  pub fn build(self, output_path: &Path) -> Result<()> {
    // Create directory
    fs::create_dir_all(output_path)?;

    // Initialize git repository
    Command::new("git").args(["init", "--initial-branch=master"]).current_dir(output_path).output()?;
    disable_background_maintenance(output_path);

    // Configure git
    Command::new("git").args(["config", "user.name", "Test User"]).current_dir(output_path).output()?;

    Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(output_path).output()?;

    // Set branch prefix if specified
    if let Some(prefix) = &self.branch_prefix {
      Command::new("git").args(["config", "branchdeck.branchPrefix", prefix]).current_dir(output_path).output()?;
    }
    for (key, value) in &self.config {
      Command::new("git").args(["config", key, value]).current_dir(output_path).output()?;
    }

    // Track if we have any commits
    let has_commits = !self.commits.is_empty();

    // Create commits
    for commit in self.commits {
      // Write files, then stage them all at once
      for (file_path, content) in &commit.files {
        let full_path = output_path.join(file_path);
        if let Some(parent) = full_path.parent() {
          fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;
      }
      Command::new("git")
        .args(["add", "--"])
        .args(commit.files.iter().map(|(file_path, _)| file_path))
        .current_dir(output_path)
        .output()?;

      // Commit with optional timestamp
      let mut cmd = Command::new("git");

      // If timestamp is provided, set GIT_AUTHOR_DATE and GIT_COMMITTER_DATE
      if let Some(ts) = commit.timestamp {
        let date_str = format!("{ts} +0000");
        cmd.env("GIT_AUTHOR_DATE", &date_str);
        cmd.env("GIT_COMMITTER_DATE", &date_str);
      }

      cmd.args(["commit", "-m", &commit.message]).current_dir(output_path).output()?;
    }

    // Add a fake origin remote pointing to self for testing
    Command::new("git").args(["remote", "add", "origin", "."]).current_dir(output_path).output()?;

    // For templates that simulate existing repositories with history,
    // create origin/master at the initial commit. This is needed for
    // sync_branches to work properly.
    if has_commits {
      // Get the initial commit
      let output = Command::new("git").args(["rev-list", "--max-parents=0", "HEAD"]).current_dir(output_path).output()?;

      let initial_commit = String::from_utf8(output.stdout)?.trim().to_string();

      // Create origin/master at the initial commit
      Command::new("git")
        .args(["update-ref", "refs/remotes/origin/master", &initial_commit])
        .current_dir(output_path)
        .output()?;
    }

    Ok(())
  }
}

/// Pre-defined test repository templates
pub mod templates {
  use super::RepoTemplate;
  use crate::git_test_utils::disable_background_maintenance;
  use anyhow::Result;
  use std::fs;
  use std::path::Path;
  use std::process::Command;

  // File contents of the conflict scenarios, successive versions of one Kotlin project
  const BUILD_GRADLE: &str = include_str!("../fixtures/conflict/build.gradle.kts");
  const BUILD_GRADLE_BCRYPT: &str = include_str!("../fixtures/conflict/build.gradle.bcrypt.kts");
  const USER_SERVICE_PATH: &str = "src/main/kotlin/com/example/service/UserService.kt";
  const USER_SERVICE: &str = include_str!("../fixtures/conflict/UserService.kt");
  const USER_SERVICE_AUTH: &str = include_str!("../fixtures/conflict/UserService.auth.kt");
  const USER_SERVICE_CACHE: &str = include_str!("../fixtures/conflict/UserService.cache.kt");
  const USER_SERVICE_JWT: &str = include_str!("../fixtures/conflict/UserService.jwt.kt");
  const USER_SERVICE_BCRYPT: &str = include_str!("../fixtures/conflict/UserService.bcrypt.kt");
  const APP_CONFIG: &str = include_str!("../fixtures/conflict/AppConfig.kt");
  const ROLES: &str = include_str!("../fixtures/conflict/Roles.kt");

  const ISSUE_LINKS_VCS_XML: &str = include_str!("../fixtures/issue_links/vcs.xml");

  /// Simple repository with 2 commits using branch prefix
  pub fn simple() -> RepoTemplate {
    simple_template(true)
  }

  /// Simple repository without branch prefix configured (for testing Welcome Card Step 2)
  pub fn simple_no_prefix() -> RepoTemplate {
    simple_template(false)
  }

  fn simple_template(with_branch_prefix: bool) -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 14:00:00 UTC and 14:30:00 UTC
    let name = if with_branch_prefix { "simple" } else { "simple_no_prefix" };
    let mut template = RepoTemplate::new(name);

    if with_branch_prefix {
      template = template.branch_prefix("user-name");
    }

    template
      .commit_with_timestamp("(test-branch) foo 1", &[("file1.txt", "Content 1")], Some(1704117600))
      .commit_with_timestamp("(test-branch) foo 2", &[("file2.txt", "Content 2")], Some(1704119400))
  }

  /// Repository with unassigned commits (no branch prefix in commit messages)
  pub fn unassigned() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("unassigned")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial commit", &[("README.md", "# Test Project\n\nThis is a test project.")], Some(1704117600))
      .commit_with_timestamp(
        "Add user authentication",
        &[("auth.js", "// Authentication logic\nexport function login() {}")],
        Some(1704119400),
      )
      .commit_with_timestamp(
        "Fix login validation bug",
        &[("auth.js", "// Authentication logic\nexport function login() {\n  // Fixed validation\n}")],
        Some(1704121200),
      )
  }

  /// Create a proper conflict scenario with interleaved commits
  fn create_conflict_scenario(name: &str, scenario_type: &str) -> RepoTemplate {
    // Use fixed timestamps starting at Jan 1, 2024 14:00:00 UTC, incrementing by 30 minutes
    let mut template = RepoTemplate::new(name)
      .branch_prefix("user-name")
      // Initial commit - this becomes origin/master
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("README.md", "# Test Repository\n\nGenerated test repository for Branch Deck conflict demonstration.\n"),
          (".gitignore", "*.class\nbuild/\n.gradle/\n.idea/\n*.iml\n"),
          ("build.gradle.kts", BUILD_GRADLE),
        ],
        Some(1704117600),
      )
      // Unassigned commit: pushed in the unassigned scenario, the base of the branches in the other
      .commit_with_timestamp("Add UserService", &[(USER_SERVICE_PATH, USER_SERVICE)], Some(1704119400))
      // feature-auth commit that syncs cleanly
      .commit_with_timestamp(
        "(feature-auth) Add authentication to UserService",
        &[(USER_SERVICE_PATH, USER_SERVICE_AUTH)],
        Some(1704121200),
      );

    match scenario_type {
      "unassigned" => {
        // Create unassigned commits scenario. Its bcrypt UserService has unindented blank lines, unlike the branches
        // scenario; keep them so the generated repository and its commit hashes stay the same
        let user_service_bcrypt = USER_SERVICE_BCRYPT.replace("\n    \n", "\n\n");
        template = template
          // Cache feature commits
          .commit_with_timestamp("(feature-cache) Add caching to UserService", &[(USER_SERVICE_PATH, USER_SERVICE_CACHE)], Some(1704123000))
          // Auth depends on cache - will conflict when grouped
          .commit_with_timestamp("(feature-auth) Add JWT tokens using cache", &[(USER_SERVICE_PATH, USER_SERVICE_JWT)], Some(1704124800))
          // Add unassigned commits that depend on each other (for conflict testing)
          .commit_with_timestamp("Add bcrypt dependency", &[("build.gradle.kts", BUILD_GRADLE_BCRYPT)], Some(1704124800))
          .commit_with_timestamp("Implement secure password hashing", &[(USER_SERVICE_PATH, user_service_bcrypt.as_str())], Some(1704126600))
          // Unassigned refactoring
          .commit_with_timestamp(
            "Refactor: Extract configuration constants",
            &[("src/main/kotlin/com/example/config/AppConfig.kt", APP_CONFIG)],
            Some(1704128400),
          );
      }
      "branches" => {
        // Create branches with conflicts scenario - matching E2E test expectations
        template = template
          .commit_with_timestamp(
            "(feature-auth) Add user roles and permissions",
            &[("src/main/kotlin/com/example/model/Roles.kt", ROLES)],
            Some(1704123000),
          )
          // Unassigned commit (bcrypt dependency) - expected by E2E test
          .commit_with_timestamp("Add bcrypt dependency", &[("build.gradle.kts", BUILD_GRADLE_BCRYPT)], Some(1704124800))
          // bug-fix branch that depends on bcrypt (will conflict)
          .commit_with_timestamp("(bug-fix) Implement secure password hashing", &[(USER_SERVICE_PATH, USER_SERVICE_BCRYPT)], Some(1704126600));
      }
      _ => unreachable!(),
    }

    template
  }

  /// Repository with unassigned commits that will have missing commits when assigned
  pub fn conflict_unassigned() -> RepoTemplate {
    create_conflict_scenario("conflict_unassigned", "unassigned")
  }

  /// Repository with branches where some commits are missing prefixes
  pub fn conflict_branches() -> RepoTemplate {
    create_conflict_scenario("conflict_branches", "branches")
  }

  /// Repository with exactly one unassigned commit for testing singular form
  pub fn single_unassigned() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("single_unassigned")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial setup", &[("README.md", "# Project\n\nInitial project setup.")], Some(1704117600))
      .commit_with_timestamp(
        "(feature) Add authentication",
        &[("auth.js", "// Authentication module\nexport function authenticate() {}")],
        Some(1704119400),
      )
      .commit_with_timestamp("Fix critical bug", &[("bugfix.txt", "Critical bug fix for production issue")], Some(1704121200)) // This is the single unassigned commit
  }

  /// Branches with long names and long commit subjects, for checking that the branch table
  /// gives the commit title the free width and truncates only the title (issue #72).
  pub fn long_subjects() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("long_subjects")
      .branch_prefix("user-name")
      .commit_with_timestamp(
        "(IJPL-219027) SettingSyncPluginManager: add setting sync plugin manager that installs, enables and reconciles plugins across machines",
        &[("plugin-manager.kt", "class SettingSyncPluginManager")],
        Some(1704117600),
      )
      .commit_with_timestamp(
        "(IJPL-219027) feat(settings-sync): persist the plugin state snapshot between restarts and apply it on startup",
        &[("plugin-state.kt", "class PluginStateSnapshot")],
        Some(1704119400),
      )
      .commit_with_timestamp(
        "(IJPL-215166) test: add UI test that enables a plugin with a dependency on an optional module and verifies the restart prompt",
        &[("plugin-ui-test.kt", "class EnablePluginTest")],
        Some(1704121200),
      )
      .commit_with_timestamp(
        "(IJPL-219674) build: add intellij.platform.plugins.parser.impl to the project model and wire it into the platform loader",
        &[("project-model.xml", "<module name=\"intellij.platform.plugins.parser.impl\" />")],
        Some(1704123000),
      )
      .commit_with_timestamp(
        "(a-really-long-branch-name-for-a-small-refactoring) refactor: narrow down types for buildPluginIdMap so that callers no longer need unchecked casts",
        &[("plugin-id-map.kt", "fun buildPluginIdMap()")],
        Some(1704124800),
      )
      .commit_with_timestamp("(docs) fix typo", &[("README.md", "# Docs")], Some(1704126600))
  }

  /// An issue branch whose commits are independent, so any of them can be split off into its own branch,
  /// a prefix branch and an unassigned commit
  pub fn issue_split() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("issue_split")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Parser\n")], Some(1704117600))
      .commit_with_timestamp("ABC-123: Add parser", &[("parser.js", "export function parse() {}")], Some(1704119400))
      .commit_with_timestamp("ABC-123: Add lexer", &[("lexer.js", "export function lex() {}")], Some(1704121200))
      .commit_with_timestamp("ABC-123: Add tokens", &[("tokens.js", "export const tokens = []")], Some(1704123000))
      .commit_with_timestamp("(docs) Update documentation", &[("docs.md", "# Documentation")], Some(1704124800))
      .commit_with_timestamp("Refactor build script", &[("build.js", "export function build() {}")], Some(1704126600))
  }

  /// Fixup commits: of a branch commit (`auth`), a `squash!` without a prefix of another branch's commit (`docs`), one
  /// without a target in the range, and an unassigned one of an unassigned commit
  pub fn fixups() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("fixups")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Login\n")], Some(1704117600))
      .commit_with_timestamp("(auth) Add login form", &[("login.js", "export function login() {}\n")], Some(1704119400))
      .commit_with_timestamp("(docs) Write the guide", &[("guide.md", "# Guide\n")], Some(1704121200))
      .commit_with_timestamp("Add build script", &[("build.js", "export function build() {}\n")], Some(1704123000))
      .commit_with_timestamp("fixup! (auth) Add login form", &[("login.js", "export function login(user) {}\n")], Some(1704124800))
      .commit_with_timestamp("squash! Write the guide\n\nCover the setup", &[("guide.md", "# Guide\n\n## Setup\n")], Some(1704126600))
      .commit_with_timestamp(
        "fixup! (auth) Drop the session cookie",
        &[("session.js", "export const session = null\n")],
        Some(1704128400),
      )
      .commit_with_timestamp("fixup! Add build script", &[("build.js", "export function build(target) {}\n")], Some(1704130200))
  }

  /// Stacked branches on: `login` stacks on `auth` and edits its file, `oauth` stacks on `login`, `session` (an issue
  /// key in its subject) on `auth`, and `docs` stands alone
  pub fn stacked_branches() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("stacked_branches")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Auth\n")], Some(1704117600))
      .commit_with_timestamp("(auth) Add auth module", &[("auth.js", "export function auth() {}\n")], Some(1704119400))
      .commit_with_timestamp(
        "(auth/login) Add login form",
        &[("auth.js", "export function auth(user) {}\n"), ("login.js", "export function login() {}\n")],
        Some(1704121200),
      )
      .commit_with_timestamp("(login/oauth) Add OAuth provider", &[("oauth.js", "export function oauth() {}\n")], Some(1704123000))
      .commit_with_timestamp(
        "(auth/session) ABC-1: Add session store",
        &[("session.js", "export const session = {}\n")],
        Some(1704124800),
      )
      .commit_with_timestamp("(docs) Document the login", &[("docs.md", "# Login\n")], Some(1704126600))
  }

  /// Stacked branches on, with a parent (`auth`) that changes a file only an unassigned commit creates: its copy
  /// conflicts, so its child (`login`) is blocked
  pub fn stacked_conflict() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("stacked_conflict")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Auth\n")], Some(1704117600))
      .commit_with_timestamp("Add config", &[("config.js", "export const config = {}\n")], Some(1704119400))
      .commit_with_timestamp("(auth) Read the auth config", &[("config.js", "export const config = { auth: true }\n")], Some(1704121200))
      .commit_with_timestamp("(auth/login) Add login form", &[("login.js", "export function login() {}\n")], Some(1704123000))
      .commit_with_timestamp("(docs) Document the login", &[("docs.md", "# Login\n")], Some(1704124800))
  }

  /// Repository with issue navigation configuration for testing issue links
  pub fn issue_links() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("issue_links")
      .branch_prefix("user-name")
      // Initial commit with IntelliJ IDEA issue navigation config
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("README.md", "# Test Repository\n\nRepository for testing issue link navigation.\n"),
          (".idea/vcs.xml", ISSUE_LINKS_VCS_XML),
        ],
        Some(1704117600),
      )
      // Commits with various issue references
      .commit_with_timestamp(
        "(feature-auth) JIRA-123: Add authentication service",
        &[("auth.js", "// Auth service for JIRA-123\nexport function authenticate() {}")],
        Some(1704119400),
      )
      .commit_with_timestamp(
        "(feature-api) Fix API endpoint for GH-456",
        &[("api.js", "// Fix for issue GH-456\nexport function fixedApi() {}")],
        Some(1704121200),
      )
      .commit_with_timestamp(
        "(feature-ui) Update UI components (#789)",
        &[("ui.js", "// UI update for issue #789\nexport function updateUI() {}")],
        Some(1704123000),
      )
      .commit_with_timestamp(
        "(feature-db) TEST-001 and PROD-999: Database optimization",
        &[("db.js", "// Optimizations for TEST-001 and PROD-999\nexport function optimizeDB() {}")],
        Some(1704124800),
      )
      // Commit without any issue references
      .commit_with_timestamp(
        "(feature-docs) Update documentation",
        &[("docs.md", "# Documentation\n\nUpdated project documentation.")],
        Some(1704126600),
      )
      // Multi-commit issue branch ABC-123 (4 commits)
      .commit_with_timestamp(
        "(ABC-123) Add user authentication module",
        &[("auth/module.js", "// User authentication module\nexport class AuthModule {}")],
        Some(1704128400),
      )
      .commit_with_timestamp(
        "(ABC-123) Add password hashing utility",
        &[("auth/hash.js", "// Password hashing utility\nexport function hashPassword() {}")],
        Some(1704130200),
      )
      .commit_with_timestamp(
        "(ABC-123) Add session management",
        &[("auth/session.js", "// Session management\nexport class SessionManager {}")],
        Some(1704132000),
      )
      .commit_with_timestamp(
        "(ABC-123) Add authentication tests",
        &[("test/auth.test.js", "// Authentication tests\ntest('should authenticate user', () => {})")],
        Some(1704133800),
      )
      // Multi-commit issue branch JIRA-456 (3 commits)
      .commit_with_timestamp(
        "(JIRA-456) Fix database connection pool timeout",
        &[("db/pool.js", "// Database connection pool\nexport const pool = { timeout: 30000 }")],
        Some(1704135600),
      )
      .commit_with_timestamp(
        "(JIRA-456) perf db: increase pool size to 50",
        &[("db/pool.js", "// Database connection pool\nexport const pool = { timeout: 30000, size: 50 }")],
        Some(1704137400),
      )
      // Multi-line body: hard-wrapped paragraph, list with a continuation line, indented code, a run-in label with code
      // tokens and a trailer
      .commit_with_timestamp(
        "(JIRA-456) Add connection retry logic

The pool gave up after the first failed connection attempt, so a short
database restart surfaced as an error for every request that was in
flight at that moment.

Retry the connection instead:
- wait with exponential backoff, starting at 100 ms and capped at 5 s,
  for up to 6 attempts
- retry only transient errors (connection refused or reset)
- log every retry at debug level

The limits can be tuned in the pool configuration:

    retry: { attempts: 6, maxDelayMs: 5000 }

Why: createPool in db/pool.js ignored POOL_RETRY_LIMIT, so `retry` never
ran, unlike the JavaScript client.

Co-Authored-By: Jane Doe <jane@example.com>",
        &[("db/retry.js", "// Connection retry logic\nexport function retryConnection() {}")],
        Some(1704139200),
      )
      // Unassigned commit with issue reference and a body (the full-message toggle in a selectable list)
      .commit_with_timestamp(
        "Emergency fix for CRITICAL-111

Skip the cache warm-up that hangs on startup until CRITICAL-112 is fixed.",
        &[("fix.js", "// Emergency fix for CRITICAL-111\nexport function emergencyFix() {}")],
        Some(1704141000),
      )
  }

  /// Directory without git initialization - for testing invalid repository paths
  pub fn empty_non_git() -> EmptyNonGitTemplate {
    EmptyNonGitTemplate
  }

  /// Template that creates a directory without git initialization
  pub struct EmptyNonGitTemplate;

  impl EmptyNonGitTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      // Create directory
      fs::create_dir_all(output_path)?;

      // Add a simple file so the directory is not empty
      fs::write(output_path.join("README.txt"), "This is not a git repository")?;

      Ok(())
    }
  }

  /// Repository with commits and uncommitted changes for testing amend functionality
  pub struct AmendChangesTemplate;

  pub fn amend_changes() -> AmendChangesTemplate {
    AmendChangesTemplate
  }

  impl AmendChangesTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      // First create the base repository with commits
      let base_template = RepoTemplate::new("amend_changes")
        .branch_prefix("user-name")
        .commit_with_timestamp("Initial setup", &[("README.md", "# Test Project\n\nInitial project setup.")], Some(1704117600))
        .commit_with_timestamp(
          "(feature-auth) Add authentication service",
          &[(
            "src/auth.js",
            "// Authentication service\nexport function authenticate(user, password) {\n  return user && password;\n}",
          )],
          Some(1704119400),
        )
        .commit_with_timestamp(
          "(feature-api) Add API endpoints",
          &[("src/api.js", "// API endpoints\nexport function getUsers() {\n  return [];\n}")],
          Some(1704121200),
        );

      // Build the base repository
      base_template.build(output_path)?;

      // Now add uncommitted changes (both staged and unstaged)

      // Add a new file (unstaged)
      let new_file_path = output_path.join("src/utils.js");
      if let Some(parent) = new_file_path.parent() {
        fs::create_dir_all(parent)?;
      }
      fs::write(
        &new_file_path,
        "// Utility functions\nexport function formatDate(date) {\n  return date.toISOString();\n}\n\nexport function validateEmail(email) {\n  return email.includes('@');\n}",
      )?;

      // Modify existing file (unstaged)
      let existing_file_path = output_path.join("src/auth.js");
      fs::write(
        &existing_file_path,
        "// Enhanced authentication service\nexport function authenticate(user, password) {\n  if (!user || !password) {\n    return false;\n  }\n  return user.length > 0 && password.length >= 8;\n}\n\nexport function logout(user) {\n  // New logout functionality\n  console.log('Logging out user:', user);\n}",
      )?;

      // Stage the new file
      Command::new("git").args(["add", "src/utils.js"]).current_dir(output_path).output()?;

      Ok(())
    }
  }

  /// What the upstream of an `UpdateTemplate` has done to the clone
  #[derive(Clone, Copy, PartialEq, Eq)]
  pub enum UpdateScenario {
    /// The local commits rebase cleanly; an unrelated file has uncommitted changes, which the update keeps
    BehindUpstream,
    /// A local commit changes the line that the upstream changed
    Conflict,
    /// The local commits rebase cleanly, but the file the upstream changed has uncommitted changes
    Blocked,
    /// A robot pushed `feature-auth` and the first of the two `search` commits: the upstream has them rebased, without
    /// the prefix, with their author and author time and the committer `Patronus`; `ui` stays local
    Landed,
  }

  /// A clone whose `master` tracks `origin/master` of an upstream that has moved on. The upstream is a bare repository
  /// at `.git/upstream.git`, the remote URL is relative, so a copy of the template fetches from its own upstream,
  /// offline. The upstream commits are already fetched, so a sync shows the update state at once.
  pub struct UpdateTemplate(UpdateScenario);

  pub fn behind_upstream() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::BehindUpstream)
  }

  pub fn update_conflict() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::Conflict)
  }

  pub fn update_blocked() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::Blocked)
  }

  pub fn landed_upstream() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::Landed)
  }

  pub const UPDATE_APP_FILE: &str = "src/app.txt";

  impl UpdateTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      let scenario = self.0;
      fs::create_dir_all(output_path)?;
      let git = |args: &[&str], timestamp: Option<i64>| -> Result<()> {
        let mut command = Command::new("git");
        if let Some(timestamp) = timestamp {
          let date = format!("{timestamp} +0000");
          command.env("GIT_AUTHOR_DATE", &date).env("GIT_COMMITTER_DATE", &date);
        }
        let output = command.args(args).current_dir(output_path).output()?;
        anyhow::ensure!(output.status.success(), "git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr));
        Ok(())
      };
      let write = |path: &str, content: &str| -> Result<()> {
        let full = output_path.join(path);
        if let Some(parent) = full.parent() {
          fs::create_dir_all(parent)?;
        }
        fs::write(full, content)?;
        Ok(())
      };
      let commit_as = |author: &str, message: &str, files: &[(&str, &str)], timestamp: i64| -> Result<()> {
        for (path, content) in files {
          write(path, content)?;
          git(&["add", "--", path], None)?;
        }
        git(&["commit", "-q", &format!("--author={author}"), "-m", message], Some(timestamp))
      };
      let commit = |message: &str, files: &[(&str, &str)], timestamp: i64| commit_as("Test User <test@example.com>", message, files, timestamp);

      git(&["init", "-q", "--initial-branch=master"], None)?;
      disable_background_maintenance(output_path);
      git(&["config", "user.name", "Test User"], None)?;
      git(&["config", "user.email", "test@example.com"], None)?;
      git(&["config", "branchdeck.branchPrefix", "user-name"], None)?;
      commit(
        "Initial commit",
        &[
          ("README.md", "# Update Project\n"),
          (UPDATE_APP_FILE, "line 1\nline 2\nline 3\n"),
          ("src/config.txt", "debug=false\n"),
        ],
        1704117600,
      )?;

      git(&["init", "-q", "--bare", "--initial-branch=master", ".git/upstream.git"], None)?;
      disable_background_maintenance(&output_path.join(".git/upstream.git"));
      git(&["remote", "add", "origin", ".git/upstream.git"], None)?;
      git(&["push", "-q", "origin", "master"], None)?;
      git(&["branch", "-q", "--set-upstream-to=origin/master", "master"], None)?;

      // upstream commits, pushed by URL so the tracking ref stays until the fetch below
      git(&["checkout", "-q", "--detach"], None)?;
      // another author, as on a shared upstream
      commit_as(
        "Dana Lee <dana@example.com>",
        "Change line 2 upstream",
        &[(UPDATE_APP_FILE, "line 1\nline 2 upstream\nline 3\n")],
        1704121200,
      )?;
      commit("Add the guide", &[("docs/guide.md", "# Guide\n")], 1704121500)?;
      if scenario == UpdateScenario::Landed {
        // as a Safe Push robot pushes the local commits below
        for (message, path, content, author_time) in [("Add login", "src/auth.txt", "login\n", 1704118500), ("Add index", "src/search.txt", "index\n", 1704118600)] {
          write(path, content)?;
          git(&["add", "--", path], None)?;
          let output = Command::new("git")
            .args(["commit", "-q", "-m", message])
            .env("GIT_AUTHOR_DATE", format!("{author_time} +0000"))
            .env("GIT_COMMITTER_DATE", "1704122000 +0000")
            .env("GIT_COMMITTER_NAME", "Patronus")
            .env("GIT_COMMITTER_EMAIL", "patronus@example.com")
            .current_dir(output_path)
            .output()?;
          anyhow::ensure!(output.status.success(), "robot commit failed: {}", String::from_utf8_lossy(&output.stderr));
        }
      }
      git(&["push", "-q", ".git/upstream.git", "HEAD:refs/heads/master"], None)?;
      git(&["checkout", "-q", "master"], None)?;

      commit("(feature-auth) Add login", &[("src/auth.txt", "login\n")], 1704118500)?;
      if scenario == UpdateScenario::Conflict {
        commit("(feature-auth) Change line 2", &[(UPDATE_APP_FILE, "line 1\nline 2 local\nline 3\n")], 1704118800)?;
      }
      if scenario == UpdateScenario::Landed {
        commit("(search) Add index", &[("src/search.txt", "index\n")], 1704118600)?;
        commit("(search) Add query", &[("src/search.txt", "index\nquery\n")], 1704118700)?;
        commit("(ui) Add button", &[("src/button.txt", "button\n")], 1704118800)?;
      }
      git(&["fetch", "-q", "origin"], None)?;

      match scenario {
        UpdateScenario::BehindUpstream => write("src/config.txt", "debug=true\n")?,
        UpdateScenario::Blocked => write(UPDATE_APP_FILE, "line 1\nline 2\nline 3\nlocal edit\n")?,
        UpdateScenario::Conflict | UpdateScenario::Landed => {}
      }
      Ok(())
    }
  }

  /// Repository with archived branches (integrated, partially integrated, and not integrated)
  /// Implements its own build to create archived refs under user-name/archived/<date>/...
  pub struct ArchivedBranchesTemplate;

  pub fn archived_branches() -> ArchivedBranchesTemplate {
    ArchivedBranchesTemplate
  }

  impl ArchivedBranchesTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      // Initialize repo
      fs::create_dir_all(output_path)?;
      Command::new("git").args(["init", "--initial-branch=main"]).current_dir(output_path).output()?;
      disable_background_maintenance(output_path);
      Command::new("git").args(["config", "user.name", "Test User"]).current_dir(output_path).output()?;
      Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(output_path).output()?;
      Command::new("git")
        .args(["config", "branchdeck.branchPrefix", "user-name"])
        .current_dir(output_path)
        .output()?;

      // Helper closures
      let write_file = |rel: &str, content: &str| -> Result<()> {
        let full = output_path.join(rel);
        if let Some(parent) = full.parent() {
          fs::create_dir_all(parent)?;
        }
        fs::write(full, content)?;
        Ok(())
      };
      let git_add_all = || -> Result<()> {
        Command::new("git").args(["add", "."]).current_dir(output_path).output()?;
        Ok(())
      };
      let git_commit = |message: &str, ts: i64| -> Result<()> {
        let date_str = format!("{ts} +0000");
        let mut cmd = Command::new("git");
        cmd.env("GIT_AUTHOR_DATE", &date_str);
        cmd.env("GIT_COMMITTER_DATE", &date_str);
        cmd.args(["commit", "-m", message]).current_dir(output_path).output()?;
        Ok(())
      };

      // Timestamps (fixed)
      let t0 = 1704117600; // B0
      let t1 = 1704117900; // F1
      let t2 = 1704118200; // F2
      let t3 = 1704118500; // P1

      // B0: initial
      write_file("README.md", "# Test Repo for Archived Branches\n")?;
      write_file("src/app.txt", "init\n")?;
      git_add_all()?;
      git_commit("Initial commit", t0)?;

      // F1: full step 1 (integrated)
      write_file("src/full.txt", "full step 1\n")?;
      git_add_all()?;
      git_commit("full step 1", t1)?;

      // F2: full step 2 (integrated)
      write_file("src/full.txt", "full step 2\n")?;
      git_add_all()?;
      git_commit("full step 2", t2)?;

      // P1: partial step 1 (integrated only first)
      write_file("src/partial.txt", "partial step 1\n")?;
      git_add_all()?;
      git_commit("partial step 1", t3)?;

      // Add origin remote and set origin/main to HEAD (current state with all commits)
      Command::new("git").args(["remote", "add", "origin", "."]).current_dir(output_path).output()?;
      let current_head = String::from_utf8(Command::new("git").args(["rev-parse", "HEAD"]).current_dir(output_path).output()?.stdout)?;
      let current_head = current_head.trim();
      Command::new("git")
        .args(["update-ref", "refs/remotes/origin/main", current_head])
        .current_dir(output_path)
        .output()?;

      // Create archived branches with their own histories, starting from the initial commit
      let date = "2025-01-11";

      // helper: checkout a detached state at initial commit
      let co_initial = || -> Result<()> {
        // get initial
        let init = String::from_utf8(Command::new("git").args(["rev-list", "--max-parents=0", "HEAD"]).current_dir(output_path).output()?.stdout)?;
        let init = init.trim().to_string();
        Command::new("git").args(["checkout", "-f", &init]).current_dir(output_path).output()?;
        Ok(())
      };
      let create_archived_from = |branch_name: &str, steps: &[(&str, &str, &str, i64)]| -> Result<()> {
        co_initial()?;
        // create a temp branch
        let tmp = format!("tmp-{branch_name}");
        Command::new("git").args(["checkout", "-b", &tmp]).current_dir(output_path).output()?;
        for (path, content, message, ts) in steps {
          write_file(path, content)?;
          git_add_all()?;
          git_commit(message, *ts)?;
        }
        // Use the expected archived prefix as per E2E docs and snapshots
        let full_ref = format!("user-name/archived/{date}/{branch_name}");
        Command::new("git").args(["branch", &full_ref, "HEAD"]).current_dir(output_path).output()?;
        // return to master
        Command::new("git").args(["checkout", "-f", "main"]).current_dir(output_path).output()?;
        // delete temp branch
        Command::new("git").args(["branch", "-D", &tmp]).current_dir(output_path).output()?;
        Ok(())
      };

      // feature-full archived (fully integrated): point archived ref to current main HEAD
      // This results in zero right-side commits relative to main
      let archived_full_ref = format!("user-name/archived/{date}/feature-full");
      Command::new("git").args(["branch", &archived_full_ref, "HEAD"]).current_dir(output_path).output()?;

      // feature-partial archived: first commit matches baseline P1, second is new
      let p1a = 1704118500; // align with P1
      let p2a = 1704118800; // new
      create_archived_from(
        "feature-partial",
        &[
          ("src/partial.txt", "partial step 1\n", "partial step 1", p1a),
          ("src/partial.txt", "partial step 2\n", "partial step 2", p2a),
        ],
      )?;

      // feature-pending archived: new content only (not on baseline)
      let n1 = 1704119100;
      create_archived_from("feature-pending", &[("src/pending.txt", "pending work\n", "pending work", n1)])?;

      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;
  use test_log::test;

  #[test]
  fn test_simple_template() {
    let temp_dir = TempDir::new().unwrap();
    let template = templates::simple();

    template.build(temp_dir.path()).unwrap();

    // Verify git repo exists
    assert!(temp_dir.path().join(".git").exists());

    // Verify files exist
    assert!(temp_dir.path().join("file1.txt").exists());
    assert!(temp_dir.path().join("file2.txt").exists());
  }
}
