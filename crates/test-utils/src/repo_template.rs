use crate::git_test_utils::{NO_BACKGROUND_MAINTENANCE, append_config, git_command};
use crate::history::{HistoryCommit, HistoryStream};
use crate::remote_git::add_bare_origin;
use anyhow::Result;
use std::fs;
use std::path::Path;

/// A builder for creating test repository templates
pub struct RepoTemplate {
  #[expect(dead_code, reason = "names the template where it is built, for the reader; nothing reads it")]
  name: String,
  branch_prefix: Option<String>,
  /// Other git config, e.g. `branchdeck.stacks`
  config: Vec<(String, String)>,
  commits: Vec<CommitSpec>,
  /// `KEY<TAB>name` lines of the issue groups (`refs/branch-deck/issue-groups`), written after the commits
  issue_groups: Vec<String>,
  /// `child<TAB>parent` lines of the stack parents (`refs/branch-deck/stack-parents`), written after the commits
  stack_parents: Vec<String>,
}

struct CommitSpec {
  message: String,
  files: Vec<(String, String)>,
  /// Paths the commit deletes
  deleted: Vec<String>,
  timestamp: Option<i64>,
}

impl RepoTemplate {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      branch_prefix: None,
      config: Vec::new(),
      commits: Vec::new(),
      issue_groups: Vec::new(),
      stack_parents: Vec::new(),
    }
  }

  /// Put the commits of issue `key` into the branch `name`, as `bd group` does with `issue_groups`
  #[must_use]
  pub fn issue_group(mut self, key: &str, name: &str) -> Self {
    self.issue_groups.push(format!("{key}\t{name}"));
    self
  }

  /// Stack the branch `child` on `parent` in the stack parents, as `bd restack` does
  #[must_use]
  pub fn stack_parent(mut self, child: &str, parent: &str) -> Self {
    self.stack_parents.push(format!("{child}\t{parent}"));
    self
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
  pub fn commit_with_timestamp(self, message: impl Into<String>, files: &[(&str, &str)], timestamp: Option<i64>) -> Self {
    self.commit_deleting(message, files, &[], timestamp)
  }

  /// A commit that writes `files` and deletes the paths of `deleted`
  #[must_use]
  pub fn commit_deleting(mut self, message: impl Into<String>, files: &[(&str, &str)], deleted: &[&str], timestamp: Option<i64>) -> Self {
    let files = files.iter().map(|(path, content)| (path.to_string(), content.to_string())).collect();

    self.commits.push(CommitSpec {
      message: message.into(),
      files,
      deleted: deleted.iter().map(ToString::to_string).collect(),
      timestamp,
    });
    self
  }

  /// Build the repository at the specified path
  pub fn build(self, output_path: &Path) -> Result<()> {
    // Create directory
    fs::create_dir_all(output_path)?;

    // Initialize git repository
    git_command().args(["init", "--initial-branch=master", "--template="]).current_dir(output_path).output()?;
    // the config in one write, not a process per key
    let mut config = NO_BACKGROUND_MAINTENANCE.to_vec();
    config.extend([("user.name", "Test User"), ("user.email", "test@example.com")]);
    if let Some(prefix) = &self.branch_prefix {
      config.push(("branchdeck.branchPrefix", prefix));
    }
    config.extend(self.config.iter().map(|(key, value)| (key.as_str(), value.as_str())));
    append_config(output_path, &config);

    // the whole history in one `fast-import` and one checkout (`history`), not an `add` and a `commit` per commit
    let mut history = HistoryStream::new("refs/heads/master");
    for commit in &self.commits {
      history.commit(&HistoryCommit {
        message: commit.message.clone(),
        files: commit.files.clone(),
        deleted: commit.deleted.clone(),
        timestamp: commit.timestamp,
        identity: None,
      });
    }
    // for templates that simulate existing repositories with history, `origin/master` at the initial commit: the
    // sync needs a baseline
    history.reset_to_root("refs/remotes/origin/master");
    let stores = [
      (&self.issue_groups, "refs/branch-deck/issue-groups", "issue-groups", "Issue groups of Branch Deck"),
      (&self.stack_parents, "refs/branch-deck/stack-parents", "stack-parents", "Stack parents of Branch Deck"),
    ];
    for (lines, store_ref, file, message) in stores.into_iter().filter(|(lines, ..)| !lines.is_empty()) {
      let mut lines = lines.clone();
      lines.sort();
      history.root_commit(store_ref, message, file, &format!("{}\n", lines.join("\n")));
    }
    history.write(output_path)?;

    // the remote a clone came from, its `master` at `origin/master`: a push reaches it, a fetch reads it
    add_bare_origin(output_path, "master")
  }
}

/// Pre-defined test repository templates
pub mod templates {
  use super::RepoTemplate;
  use crate::git_test_utils::{NO_BACKGROUND_MAINTENANCE, append_config, disable_background_maintenance, git_command};
  use crate::remote_git::{BARE_ORIGIN, add_bare_origin};
  use anyhow::Result;
  use std::fmt::Write;
  use std::fs;
  use std::path::Path;

  /// The big stack of `stack_island` (and of each stack of `branch_table_large`): the chain from `air` to `deploy` with
  /// the side chain of `side-fix` and `side-test` off `bazel`, as `(prefix, subject)` of its commits
  const STACK_ISLAND_BRANCHES: [(&str, &str); 7] = [
    ("air", "Add the agent"),
    ("air/bazel", "Build the deploy jar"),
    ("bazel/bridge", "Start the resident bridge"),
    ("bridge/runtime", "Load the control runtime"),
    ("bazel/side-fix", "Fix the jar manifest"),
    ("runtime/deploy", "Deploy the runtime"),
    ("side-fix/side-test", "Test the manifest"),
  ];

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

  /// A Kotlin file whose class name is on its first line and in its last function, 20 lines apart
  fn row_mapper(class_name: &str) -> String {
    let fields = (1..=20).fold(String::new(), |mut fields, i| {
      let _ = writeln!(fields, "  val field{i} = {i}");
      fields
    });
    format!("class {class_name}Mapper {{\n{fields}  fun map(state: String): {class_name} = {class_name}(state)\n}}\n")
  }

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
  /// gives the commit title the free width and truncates only the title (issue #72). `docs` has a description, as
  /// `bd describe` writes it
  pub fn long_subjects() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("long_subjects")
      .branch_prefix("user-name")
      .config("branch.user-name/virtual/docs.description", "docs: fix the typo in the README\n\nThe heading said Dosc.\n")
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

  /// One commit that changes files in nested directories (Kotlin, a Gradle script, the README), for the directory tree
  /// of the commit preview: `RowMapper.kt` has two hunks with 20 unchanged lines between them, which a diff can expand;
  /// the changed line of `Reducer.kt` is longer than a side of Split, which scrolls or wraps it
  pub fn commit_tree() -> RepoTemplate {
    RepoTemplate::new("commit_tree")
      .branch_prefix("user-name")
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("README.md", "# Air\n\nSessions of the agent.\n"),
          ("plugins/air/build.gradle.kts", "plugins {\n  kotlin(\"jvm\")\n}\n"),
          ("plugins/air/backend/session/catalog/src/RowMapper.kt", &row_mapper("Row")),
          (
            "plugins/air/backend/session/projection/src/Reducer.kt",
            "class Reducer {\n  fun reduce(state: Int, event: SessionEvent, clock: Clock, log: Logger, metrics: Metrics, settings: SessionSettings): Int = state\n}\n",
          ),
        ],
        Some(1704117600),
      )
      .commit_with_timestamp(
        "(session) a prompt that waits for its turn keeps the session row running",
        &[
          ("README.md", "# Air\n\nSessions of the agent, one row each.\n"),
          ("plugins/air/build.gradle.kts", "plugins {\n  kotlin(\"jvm\")\n  id(\"air.session\")\n}\n"),
          ("plugins/air/backend/session/catalog/src/RowMapper.kt", &row_mapper("SessionRow")),
          (
            "plugins/air/backend/session/catalog/testSrc/RowMapperTest.kt",
            "class RowMapperTest {\n  @Test\n  fun keepsTheRowRunning() {\n    assertEquals(\"running\", map(\"waiting\"))\n  }\n}\n",
          ),
          (
            "plugins/air/backend/session/projection/src/Reducer.kt",
            "class Reducer {\n  fun reduce(state: Int, event: SessionEvent, clock: Clock, log: Logger, metrics: Metrics, settings: SessionSettings): Int = state + 1\n}\n",
          ),
        ],
        Some(1704119400),
      )
  }

  /// One commit for Go to Declaration in a review: `Uses.kt` calls `store.add` of an untyped receiver (two `add` in the
  /// review: `Store` and `Queue`), `helper`, declared at the end of `Store.kt`, whose change at its top leaves the
  /// declaration in a fold, and `outsider` of `Base.kt`, which the commit does not change: outside the review; it calls
  /// `helper`, a declaration of the review
  pub fn symbol_navigation() -> RepoTemplate {
    let store = |version: u32| {
      let mut text = format!("class Store {{\n  val version = {version}\n");
      for index in 0..30 {
        let _ = writeln!(text, "  val field{index} = {index}");
      }
      text.push_str("  fun add(value: Int) {}\n}\n\nfun helper(value: Int) = value\n");
      text
    };
    RepoTemplate::new("symbol_navigation")
      .branch_prefix("user-name")
      .commit_with_timestamp(
        "Initial project setup",
        &[("src/Store.kt", &store(1)), ("src/Base.kt", "fun outsider(value: Int) = helper(value)\n")],
        Some(1704117600),
      )
      .commit_with_timestamp(
        "(nav) use the store and the queue",
        &[
          ("src/Store.kt", &store(2)),
          ("src/Queue.kt", "class Queue {\n  fun add(value: Int) {}\n}\n"),
          (
            "src/Uses.kt",
            "class Uses {\n  fun run(store: Any) {\n    store.add(1)\n    helper(2)\n    outsider(3)\n  }\n}\n",
          ),
        ],
        Some(1704119400),
      )
  }

  /// A branch `rename` whose one commit makes the same edit in three files of the baseline, each of them that edit alone,
  /// and the same edit with a change of its function in a fourth: the files the review plan folds as the same edit
  pub fn repeated_edit() -> RepoTemplate {
    let source = |name: &str, body: &str| format!("package a\n\nclass {name} {{\n  val manager = ProjectManager.getInstance()\n\n  fun run() = {body}\n}}\n");
    let names = ["Alpha", "Beta", "Gamma", "Delta"];
    let base: Vec<(String, String)> = names.iter().map(|name| (format!("src/a/{name}.kt"), source(name, "1"))).collect();
    let edited: Vec<(String, String)> = names
      .iter()
      .map(|name| {
        let body = if *name == "Delta" { "2" } else { "1" };
        (
          format!("src/a/{name}.kt"),
          source(name, body).replace("ProjectManager.getInstance()", "ProjectManagerEx.getInstanceEx()"),
        )
      })
      .collect();
    fn refs(files: &[(String, String)]) -> Vec<(&str, &str)> {
      files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect()
    }
    RepoTemplate::new("repeated_edit")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &refs(&base), Some(1704117600))
      .commit_with_timestamp("(rename) use the Ex manager", &refs(&edited), Some(1704119400))
  }

  /// One commit of the branch `search` beside files nobody writes by hand: the source `src/guide.md` and its two
  /// renderings with a `Generated by` header (`AGENTS.md`, `docs/AGENTS.md`) get the same edit, a JSON file that
  /// `.gitattributes` marks `linguist-generated`, a lock file and a minified file change too, and `src/App.kt` is the
  /// real work
  pub fn generated_files() -> RepoTemplate {
    let guide = |tool: &str| format!("# Guide\n\nSearch the source with {tool}.\n");
    let agents = |tool: &str| format!("<!-- Generated by tools/render.mjs; edit src/guide.md -->\n\n{}", guide(tool));
    let files = |tool: &str, lock: &str, version: u8| {
      vec![
        ("src/guide.md".to_string(), guide(tool)),
        ("AGENTS.md".to_string(), agents(tool)),
        ("docs/AGENTS.md".to_string(), agents(tool)),
        ("config/mcp.json".to_string(), format!("{{\n  \"servers\": {{\n    \"search\": \"{tool}\"\n  }}\n}}\n")),
        ("pnpm-lock.yaml".to_string(), format!("lockfileVersion: '9.0'\n\npackages:\n{lock}")),
        ("dist/app.min.js".to_string(), format!("console.log({version})\n")),
        ("src/App.kt".to_string(), format!("class App {{\n  fun search() = \"{tool}\"\n}}\n")),
      ]
    };
    let before = files("grep", "  a@1.0.0: {}\n", 1);
    let after = files("accio", "  a@1.0.0: {}\n  b@2.0.0: {}\n", 2);
    let mut initial: Vec<(&str, &str)> = before.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
    initial.push((".gitattributes", "config/*.json linguist-generated\n"));
    let changed: Vec<(&str, &str)> = after.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
    RepoTemplate::new("generated_files")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &initial, Some(1704117600))
      .commit_with_timestamp("(search) search the source with accio", &changed, Some(1704119400))
  }

  /// One commit of one TSX file for the colours IntelliJ gives a name of JavaScript by its declaration: an imported
  /// function, a parameter, a local variable, a component, a method
  pub fn script_colors() -> RepoTemplate {
    RepoTemplate::new("script_colors")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# view\n")], Some(1704117600))
      .commit_with_timestamp(
        "(colors) add the counter",
        &[(
          "src/Counter.tsx",
          "import { useState } from \"react\"\n\nexport function Counter({ start }: { start: number }) {\n  const [count, setCount] = useState(start)\n  return <Button onClick={() => setCount(count + 1)}>{count.toFixed()}</Button>\n}\n",
        )],
        Some(1704119400),
      )
  }

  /// One commit of one file for the whitespace policies of the diffs: a block of two lines indented again, a blank line
  /// added apart from the other changes, and one real change, so each policy shows a different diff (`Format.kt`)
  pub fn commit_whitespace() -> RepoTemplate {
    RepoTemplate::new("commit_whitespace")
      .branch_prefix("user-name")
      .commit_with_timestamp(
        "Initial project setup",
        &[
          (
            "src/Format.kt",
            "class Format {\n  fun first() {\n    val a = 1\n    val b = 2\n  }\n  fun middle() = 0\n  fun second() = 3\n}\n",
          ),
          ("src/Main.kt", "fun main() = Format().second()\n"),
        ],
        Some(1704117600),
      )
      .commit_with_timestamp(
        "(format) indent the block of first and return 4",
        &[
          (
            "src/Format.kt",
            "class Format {\n  fun first() {\n      val a = 1\n      val b = 2\n  }\n\n  fun middle() = 0\n  fun second() = 4\n}\n",
          ),
          ("src/Main.kt", "fun main() = println(Format().second())\n"),
        ],
        Some(1704119400),
      )
  }

  /// One commit that deletes a file and changes another, for the status of a deleted file in the tree and the review
  pub fn commit_deleted() -> RepoTemplate {
    RepoTemplate::new("commit_deleted")
      .branch_prefix("user-name")
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("src/Mapper.kt", "class Mapper {\n  fun map(row: Row): Model = Model(row)\n}\n"),
          ("src/LegacyMapper.kt", "class LegacyMapper {\n  fun map(row: Row): Model = Model(row.legacy)\n}\n"),
        ],
        Some(1704117600),
      )
      .commit_deleting(
        "(cleanup) drop the legacy mapper",
        &[("src/Mapper.kt", "class Mapper {\n  fun map(row: Row): Model = Model(row, legacy = false)\n}\n")],
        &["src/LegacyMapper.kt"],
        Some(1704119400),
      )
  }

  /// One commit of nine files in three modules, enough for the search of the files of a review
  /// (`FILE_FILTER_MIN_FILES`): `parser`, `lexer` and `printer`, each with a source, a test and a README
  pub fn commit_many_files() -> RepoTemplate {
    RepoTemplate::new("commit_many_files")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Tools\n")], Some(1704117600))
      .commit_with_timestamp(
        "(tools) every tool reads its input as a stream",
        &[
          ("parser/src/Parser.kt", "class Parser {\n  fun parse(input: Stream) = input\n}\n"),
          ("parser/testSrc/ParserTest.kt", "class ParserTest {\n  fun readsAStream() {}\n}\n"),
          ("parser/README.md", "# Parser\n\nReads a stream.\n"),
          ("lexer/src/Lexer.kt", "class Lexer {\n  fun lex(input: Stream) = input\n}\n"),
          ("lexer/testSrc/LexerTest.kt", "class LexerTest {\n  fun readsAStream() {}\n}\n"),
          ("lexer/README.md", "# Lexer\n\nReads a stream.\n"),
          ("printer/src/Printer.kt", "class Printer {\n  fun print(input: Stream) = input\n}\n"),
          ("printer/testSrc/PrinterTest.kt", "class PrinterTest {\n  fun readsAStream() {}\n}\n"),
          ("printer/README.md", "# Printer\n\nReads a stream.\n"),
        ],
        Some(1704119400),
      )
  }

  /// An issue branch whose commits are independent, so any of them can be split off into its own branch, its
  /// description titled with its key as a pull request title is; a prefix branch and an unassigned commit
  pub fn issue_split() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("issue_split")
      .branch_prefix("user-name")
      .config("branch.user-name/virtual/ABC-123.description", "ABC-123 Parse the source in one pass\n")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Parser\n")], Some(1704117600))
      .commit_with_timestamp("ABC-123: Add parser", &[("parser.js", "export function parse() {}")], Some(1704119400))
      .commit_with_timestamp("ABC-123: Add lexer", &[("lexer.js", "export function lex() {}")], Some(1704121200))
      .commit_with_timestamp("ABC-123: Add tokens", &[("tokens.js", "export const tokens = []")], Some(1704123000))
      .commit_with_timestamp("(docs) Update documentation", &[("docs.md", "# Documentation")], Some(1704124800))
      .commit_with_timestamp("Refactor build script", &[("build.js", "export function build() {}")], Some(1704126600))
  }

  /// Two issue keys in the issue group `parser`, a prefix branch and an unassigned commit
  pub fn issue_groups() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("issue_groups")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Parser\n")], Some(1704117600))
      .commit_with_timestamp("ABC-1: Add parser", &[("parser.js", "export function parse() {}")], Some(1704119400))
      .commit_with_timestamp("ABC-2: Add lexer", &[("lexer.js", "export function lex() {}")], Some(1704121200))
      .commit_with_timestamp("(docs) Update documentation", &[("docs.md", "# Documentation")], Some(1704123000))
      .commit_with_timestamp("Refactor build script", &[("build.js", "export function build() {}")], Some(1704124800))
      .issue_group("ABC-1", "parser")
      .issue_group("ABC-2", "parser")
  }

  /// With stacked branches on: two issue branches and a prefix branch stacked on the first by the stack parents,
  /// without a parent level in its prefix
  pub fn stacked_issues() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("stacked_issues")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Parser\n")], Some(1704117600))
      .commit_with_timestamp("ABC-1: Add parser", &[("parser.js", "export function parse() {}\n")], Some(1704119400))
      .commit_with_timestamp("ABC-2: Parse lists", &[("lists.js", "export function parseList() {}\n")], Some(1704121200))
      .commit_with_timestamp("(docs) Document the parser", &[("docs.md", "# Parser\n")], Some(1704123000))
      .stack_parent("docs", "ABC-1")
  }

  /// Three issue branches, one with two commits, that ship together, and a prefix branch
  pub fn issue_combine() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("issue_combine")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Maps\n")], Some(1704117600))
      .commit_with_timestamp("IJPL-1: Parse lists", &[("lists.js", "export function lists() {}")], Some(1704119400))
      .commit_with_timestamp("IJPL-1: Parse maps", &[("maps.js", "export function maps() {}")], Some(1704121200))
      .commit_with_timestamp("IJPL-2: Print maps", &[("print.js", "export function print() {}")], Some(1704123000))
      .commit_with_timestamp("IJPL-3: Print sets", &[("sets.js", "export function sets() {}")], Some(1704124800))
      .commit_with_timestamp("(docs) Update documentation", &[("docs.md", "# Documentation")], Some(1704126600))
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

  /// Stacked branches on: `auth` begins first and has the newest commit but one, a fixup of its oldest commit, `session`
  /// stacks on `auth`, `docs` and `search` begin after them; a squash of the fixup gives every commit above the oldest a
  /// new committer time
  pub fn fixup_order() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("fixup_order")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Accounts\n")], Some(1704117600))
      .commit_with_timestamp("(auth) IJAI-101 Validate the password", &[("auth.js", "export function validate() {}\n")], Some(1704119400))
      .commit_with_timestamp("(auth/session) Keep the session", &[("session.js", "export const session = {}\n")], Some(1704121200))
      .commit_with_timestamp("(docs) Write the guide", &[("guide.md", "# Guide\n")], Some(1704123000))
      .commit_with_timestamp(
        "fixup! (auth) IJAI-101 Validate the password",
        &[("auth.js", "export function validate(password) {}\n")],
        Some(1704124800),
      )
      .commit_with_timestamp("(search) Index the names", &[("search.js", "export function index() {}\n")], Some(1704126600))
  }

  /// Stacked branches on: `login` stacks on `auth` and edits its file, `oauth` stacks on `login`, `session` (an issue
  /// key in its subject) on `auth`, and `docs` stands alone
  pub fn stacked_branches() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    stacked_auth("stacked_branches")
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

  /// Stacked branches on, a straight chain: `oauth` on `login` on `auth`, one commit each, and `docs` alone; a stack of
  /// GitHub on a route without Patronus (docs/design/github-stacks.md)
  pub fn stacked_chain() -> RepoTemplate {
    stacked_auth("stacked_chain")
      .commit_with_timestamp("(auth/login) Add login form", &[("login.js", "export function login() {}\n")], Some(1704121200))
      .commit_with_timestamp("(login/oauth) Add OAuth provider", &[("oauth.js", "export function oauth() {}\n")], Some(1704123000))
      .commit_with_timestamp("(docs) Document the login", &[("docs.md", "# Login\n")], Some(1704126600))
  }

  /// The start of the templates of the `auth` stack: stacked branches on, the setup and `auth`
  fn stacked_auth(name: &str) -> RepoTemplate {
    RepoTemplate::new(name)
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Auth\n")], Some(1704117600))
      .commit_with_timestamp("(auth) Add auth module", &[("auth.js", "export function auth() {}\n")], Some(1704119400))
  }

  /// Stacked branches on, a big stack: `air` heads a chain of five branches with a side chain of two off `bazel`, two
  /// commits each, so it is an island of its own below the table; `docs` and the small stack `auth/login` stay in the
  /// table
  pub fn stack_island() -> RepoTemplate {
    let mut template = RepoTemplate::new("stack_island")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Air\n")], Some(1704117600));
    let mut time = 1704119400;
    for (prefix, subject) in STACK_ISLAND_BRANCHES {
      let leaf = prefix.rsplit('/').next().unwrap();
      for part in 1..=2 {
        let file = format!("{leaf}/part{part}.txt");
        let content = format!("{subject} {part}\n");
        template = template.commit_with_timestamp(format!("({prefix}) {subject}, part {part}"), &[(file.as_str(), content.as_str())], Some(time));
        time += 1800;
      }
    }
    template
      .commit_with_timestamp("(auth) Add auth module", &[("auth.js", "export function auth() {}\n")], Some(time))
      .commit_with_timestamp("(auth/login) Add login form", &[("login.js", "export function login() {}\n")], Some(time + 1800))
      .commit_with_timestamp("(docs) Document the agent", &[("docs.md", "# Agent\n")], Some(time + 3600))
  }

  /// The README images (`tests/screenshots/readme.shot.ts`): a notes app of `alex` with stacked branches on, the stack
  /// `sync` of four branches of two commits, `conflicts/offline` and `presence` on `sync` (small enough to stay in the
  /// table), the issue branches `ABC-123` (two commits) and `ABC-131`, `search` and `docs`: eight branches, so the filter
  /// bar shows. Kotlin sources (`fixtures/readme`) for the colours of the review. The commits interleave the branches, as
  /// the commits of a day of work do.
  pub fn readme_showcase() -> RepoTemplate {
    const ENGINE: &str = "src/sync/SyncEngine.kt";
    const STATUS_BAR: &str = "src/editor/StatusBar.kt";
    const SEARCH: &str = "src/search/FileSearch.kt";
    let commits: [(&str, &[(&str, &str)]); 13] = [
      (
        "(sync) Download only the edits after the last sync",
        &[(ENGINE, include_str!("../fixtures/readme/SyncEngine.cursor.kt"))],
      ),
      (
        "(sync) Keep the sync cursor across restarts",
        &[("src/sync/SyncCursorStore.kt", include_str!("../fixtures/readme/SyncCursorStore.kt"))],
      ),
      (
        "ABC-123 Show the upload progress in the status bar",
        &[(STATUS_BAR, include_str!("../fixtures/readme/StatusBar.progress.kt"))],
      ),
      (
        "(sync/conflicts) Merge two edits of the same paragraph word by word",
        &[("src/sync/ConflictResolver.kt", include_str!("../fixtures/readme/ConflictResolver.kt"))],
      ),
      (
        "(conflicts) Resolve the remote edits against the local ones",
        &[(ENGINE, include_str!("../fixtures/readme/SyncEngine.merge.kt"))],
      ),
      (
        "(search) Rank the notes whose file name matches first",
        &[(SEARCH, include_str!("../fixtures/readme/FileSearch.names.kt"))],
      ),
      (
        "(conflicts/offline) Queue the edits made offline",
        &[("src/sync/OfflineQueue.kt", include_str!("../fixtures/readme/OfflineQueue.kt"))],
      ),
      (
        "(offline) Upload the queued edits once the connection is back",
        &[("src/sync/ConnectionWatcher.kt", include_str!("../fixtures/readme/ConnectionWatcher.kt"))],
      ),
      (
        "ABC-123 Hide the progress once the upload is done",
        &[(STATUS_BAR, include_str!("../fixtures/readme/StatusBar.done.kt"))],
      ),
      (
        "(sync/presence) Show who else has the document open",
        &[("src/sync/Presence.kt", include_str!("../fixtures/readme/Presence.kt"))],
      ),
      (
        "(presence) Hide the editors idle for ten minutes",
        &[("src/sync/Presence.kt", include_str!("../fixtures/readme/Presence.idle.kt"))],
      ),
      (
        "ABC-131 Keep the scroll position when the file reloads",
        &[("src/editor/ScrollKeeper.kt", include_str!("../fixtures/readme/ScrollKeeper.kt"))],
      ),
      (
        "(docs) Document the keyboard shortcuts",
        &[("docs/shortcuts.md", "# Keyboard shortcuts\n\n- ⌘K: search the notes\n- ⌘S: sync now\n")],
      ),
    ];
    // Jan 1, 2024 from 08:00 UTC, 35 minutes apart
    let mut time = 1704096000;
    let mut template = RepoTemplate::new("readme_showcase")
      .branch_prefix("alex")
      .config("branchdeck.stacks", "true")
      .config("user.name", "Alex Morgan")
      .config("user.email", "alex@example.com")
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("README.md", "# Notes\n"),
          (ENGINE, include_str!("../fixtures/readme/SyncEngine.kt")),
          (STATUS_BAR, include_str!("../fixtures/readme/StatusBar.kt")),
          (SEARCH, include_str!("../fixtures/readme/FileSearch.kt")),
        ],
        Some(time),
      );
    for (subject, files) in commits {
      time += 2100;
      template = template.commit_with_timestamp(subject, files, Some(time));
    }
    template
  }

  /// The branch view at scale, for `tests/perf/branch-table.perf.ts`: stacked branches on, 120 lone branches of one
  /// commit, their subjects of four lengths, then three stacks of `STACK_ISLAND_BRANCHES`, each an island of its own below
  /// the table
  pub fn branch_table_large() -> RepoTemplate {
    const SUBJECTS: [&str; 4] = [
      "Fix the cache",
      "Read the settings of the project once it is opened",
      "Keep the order of the tool windows when the layout of the frame is restored after a restart",
      "Index the sources of the libraries of the module in the background, so the editor answers the completion at once while the scan of the jars runs and the caches of the previous session still hold",
    ];
    let mut time = 1704117600;
    let mut template = RepoTemplate::new("branch_table_large")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Large\n")], Some(time));
    for index in 0..120 {
      time += 600;
      let name = format!("feature-{index:03}");
      let file = format!("{name}.txt");
      let content = format!("{name}\n");
      template = template.commit_with_timestamp(format!("({name}) {}", SUBJECTS[index % SUBJECTS.len()]), &[(file.as_str(), content.as_str())], Some(time));
    }
    for stack in ["one", "two", "three"] {
      for (prefix, subject) in STACK_ISLAND_BRANCHES {
        time += 600;
        // `air/bazel` of the stack `two` is `two-air/two-bazel`
        let prefix = prefix.split('/').map(|part| format!("{stack}-{part}")).collect::<Vec<_>>().join("/");
        let leaf = prefix.rsplit('/').next().unwrap_or(&prefix);
        let file = format!("{leaf}.txt");
        let content = format!("{subject}\n");
        template = template.commit_with_timestamp(format!("({prefix}) {subject}"), &[(file.as_str(), content.as_str())], Some(time));
      }
    }
    template
  }

  /// Stacked branches on, a big stack whose stack branch conflicts: `air` heads the main line `bazel`, `bridge`,
  /// `runtime`, `deploy`, and `side-fix` off `bazel` changes a file only an unassigned commit creates, so its copy
  /// conflicts, and so does the stack branch `user-name/stack/air`; the main line builds
  pub fn stack_side_conflict() -> RepoTemplate {
    let mut template = RepoTemplate::new("stack_side_conflict")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Air\n")], Some(1704117600))
      .commit_with_timestamp("Add config", &[("config.js", "export const config = {}\n")], Some(1704119400));
    let branches = [
      ("air", "Add the agent"),
      ("air/bazel", "Build the deploy jar"),
      ("bazel/bridge", "Start the resident bridge"),
      ("bridge/runtime", "Load the control runtime"),
      ("runtime/deploy", "Deploy the runtime"),
    ];
    let mut time = 1704121200;
    for (prefix, subject) in branches {
      let leaf = prefix.rsplit('/').next().unwrap();
      let file = format!("{leaf}/part.txt");
      let content = format!("{subject}\n");
      template = template.commit_with_timestamp(format!("({prefix}) {subject}"), &[(file.as_str(), content.as_str())], Some(time));
      time += 1800;
    }
    template.commit_with_timestamp(
      "(bazel/side-fix) Read the jar config",
      &[("config.js", "export const config = { jar: true }\n")],
      Some(time),
    )
  }

  /// Stacked branches on, with code owners: `OWNERSHIP` files in the baseline name a team per directory; `login` stacks
  /// on `auth` and changes files of both teams and of the root owner
  pub fn stacked_ownership() -> RepoTemplate {
    RepoTemplate::new("stacked_ownership")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("README.md", "# Auth\n"),
          ("OWNERSHIP", "owner 'Monorepo'\n"),
          ("auth/OWNERSHIP", "owner 'Auth Team'\n"),
          ("ui/OWNERSHIP", "/*.css\nowner 'Design'\n\n/*.js\nowner 'UI Team'\n"),
        ],
        Some(1704117600),
      )
      .commit_with_timestamp("(auth) Add the token", &[("auth/token.js", "export const token = 1\n")], Some(1704119400))
      .commit_with_timestamp(
        "(auth/login) Add the login form",
        &[("auth/token.js", "export const token = 2\n"), ("ui/login.js", "export function login() {}\n")],
        Some(1704121200),
      )
      .commit_with_timestamp(
        "(login) Style the form",
        &[("ui/login.css", "form {}\n"), ("README.md", "# Auth and login\n")],
        Some(1704123000),
      )
  }

  /// Stacked branches on, the stack parents stack `perf-part-2` on `IJPL-257337`: the one commit of `perf-part-2`
  /// changes the file that the second commit of `IJPL-257337` creates, so it needs that commit only. `OWNERSHIP` files
  /// name a team for `icons/` and `ui/`
  pub fn stack_push_alone() -> RepoTemplate {
    RepoTemplate::new("stack_push_alone")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .stack_parent("perf-part-2", "IJPL-257337")
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("README.md", "# Icons\n"),
          ("OWNERSHIP", "owner 'Monorepo'\n"),
          ("icons/OWNERSHIP", "owner 'Icons Team'\n"),
          ("ui/OWNERSHIP", "owner 'UI Team'\n"),
        ],
        Some(1704117600),
      )
      .commit_with_timestamp("(IJPL-257337) Load icons lazily", &[("icons/loader.kt", "fun load() = lazy {}\n")], Some(1704119400))
      .commit_with_timestamp("(IJPL-257337) Parse the icon off the EDT", &[("ui/AppUIUtil.kt", "fun parse() {}\n")], Some(1704121200))
      .commit_with_timestamp(
        "(perf-part-2) Cache the parsed icon",
        &[("ui/AppUIUtil.kt", "fun parse() {}\nval cache = parse()\n")],
        Some(1704123000),
      )
  }

  /// Stacked branches (the symbols are on by default): `lane-ui` (stacked on `lane-core` by the stack parents) applies alone, but
  /// its commit calls a function the commit of `lane-core` adds in another file
  pub fn stack_uses_parent() -> RepoTemplate {
    RepoTemplate::new("stack_uses_parent")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .stack_parent("lane-ui", "lane-core")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Lane\n")], Some(1704117600))
      .commit_with_timestamp(
        "(lane-core) Add the lane document",
        &[("core/src/lane/Document.kt", "package lane\n\nfun laneDocument(): String = \"\"\n")],
        Some(1704119400),
      )
      .commit_with_timestamp(
        "(lane-ui) Show the lane document",
        &[("ui/src/view/View.kt", "package view\n\nimport lane.laneDocument\n\nfun show() = laneDocument()\n")],
        Some(1704121200),
      )
  }

  /// Stacked branches on (the symbols are on by default), the robot of 2026-10-22: `lane-run` imports a class that the
  /// commit of `stills-codec`, a branch it is not built on, adds; `ui-perf-nightly` is built on `lane-run`. git merges
  /// either, the build of its Safe Push fails: both need `stills-codec` (the landing needs of the repository view)
  pub fn landing_needs() -> RepoTemplate {
    landing_needs_named("landing_needs")
  }

  /// The commits of `landing_needs`, under `name`
  fn landing_needs_named(name: &str) -> RepoTemplate {
    RepoTemplate::new(name)
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Lanes\n")], Some(1704117600))
      .commit_with_timestamp(
        "(stills-codec) Name lossy stills",
        &[("bundle/src/Stills.kt", "package bundle\n\nclass Stills\n")],
        Some(1704119400),
      )
      .commit_with_timestamp(
        "(lane-run) Show each scenario",
        &[("lane/src/BuildLog.kt", "package lane\n\nimport bundle.Stills\n\nfun show(stills: Stills) = 1\n")],
        Some(1704121200),
      )
      .commit_with_timestamp(
        "(lane-run/ui-perf-nightly) Measure the UI nightly",
        &[("perf/src/Nightly.kt", "package perf\n\nfun nightly() = 1\n")],
        Some(1704123000),
      )
  }

  /// `landing_needs` with a fork: `lane-docs` is built on `lane-run` too, so the stack of `lane-run` forks and lands
  /// whole from its stack branch, whose build needs `stills-codec` outside it (the landing gate of the whole stack)
  pub fn landing_needs_fork() -> RepoTemplate {
    landing_needs_named("landing_needs_fork").commit_with_timestamp("(lane-run/lane-docs) Document the lanes", &[("docs/lanes.md", "# Lanes\n")], Some(1704124800))
  }

  /// Stacked branches on, with a stack problem of each kind the app offers a fix for: `publish-viewer` declares
  /// `ci-traces` by its prefix and `trace-tools` by the stack parents (conflicting parents), with `viewer-docs` stacked
  /// on it (blocked); `lane-a` and `lane-b` declare each other (a cycle); `Notes` and `notes` differ only in case
  pub fn stack_problems() -> RepoTemplate {
    RepoTemplate::new("stack_problems")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .stack_parent("publish-viewer", "trace-tools")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Traces\n")], Some(1704117600))
      .commit_with_timestamp("(ci-traces) Collect the traces", &[("ci/traces.kt", "fun traces() {}\n")], Some(1704119400))
      .commit_with_timestamp("(trace-tools) Add the trace tools", &[("tools/tools.kt", "fun tools() {}\n")], Some(1704121200))
      .commit_with_timestamp(
        "(ci-traces/publish-viewer) Publish the viewer",
        &[("viewer/publish.kt", "fun publish() {}\n")],
        Some(1704123000),
      )
      .commit_with_timestamp("(publish-viewer/viewer-docs) Document the viewer", &[("viewer/README.md", "# Viewer\n")], Some(1704124800))
      .commit_with_timestamp("(lane-b/lane-a) Add lane A", &[("lanes/a.kt", "fun a() {}\n")], Some(1704126600))
      .commit_with_timestamp("(lane-a/lane-b) Add lane B", &[("lanes/b.kt", "fun b() {}\n")], Some(1704128400))
      .commit_with_timestamp("(Notes) Write the notes", &[("notes/one.md", "one\n")], Some(1704130200))
      .commit_with_timestamp("(notes) Fix the notes", &[("notes/two.md", "two\n")], Some(1704132000))
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

  /// The commit of `ui` changes the retries on the line whose timeout `core` raised first, and adds a line: git stops
  /// at the line, but its words resolve (`{ timeout: 10, retries: 3 }`), and the added line resolves as it is. The
  /// resolve by words is off, so the sync reports the conflict of git and the view shows what the words would do
  pub fn conflict_words() -> RepoTemplate {
    words_conflict("conflict_words").config("branchdeck.resolveByWords", "false")
  }

  /// `conflict_words` with the resolve by words on, as by default: the sync copies the commit of `ui` with the result
  pub fn resolved_words() -> RepoTemplate {
    words_conflict("resolved_words")
  }

  fn words_conflict(name: &str) -> RepoTemplate {
    // the limits line, and two lines apart from the end of the file
    let limits = |timeout: u32, retries: u32, end: &str| {
      format!(
        "export const host = \"localhost\"\nexport const limits = {{ timeout: {timeout}, retries: {retries} }}\nexport const port = 8080\nexport const name = \"limits\"\n{end}"
      )
    };
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new(name)
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial project setup", &[("limits.js", limits(10, 1, "").as_str())], Some(1704117600))
      .commit_with_timestamp("(core) Raise the timeout", &[("limits.js", limits(30, 1, "").as_str())], Some(1704119400))
      .commit_with_timestamp(
        "(ui) Retry three times and log it",
        &[("limits.js", limits(30, 3, "export const verbose = true\n").as_str())],
        Some(1704121200),
      )
  }

  /// Stacked branches on: the last commit of `ui` changes a line of the settings that `api` (stacked on `core`) changed
  /// first, so its copy conflicts with markers; the conflict fix moves it to `api`
  pub fn conflict_fix() -> RepoTemplate {
    let settings = |timeout: &str| {
      format!("export const host = \"localhost\"\nexport const port = 8080\nexport const timeout = {timeout}\nexport const retries = 1\nexport const verbose = false\n")
    };
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("conflict_fix")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp(
        "Initial project setup",
        &[("README.md", "# Settings\n"), ("settings.js", settings("10").as_str())],
        Some(1704117600),
      )
      .commit_with_timestamp("(core) Add the client", &[("client.js", "export function connect() {}\n")], Some(1704119400))
      .commit_with_timestamp("(core/api) Raise the timeout", &[("settings.js", settings("30").as_str())], Some(1704121200))
      .commit_with_timestamp("(ui) Add the settings page", &[("page.js", "export function page() {}\n")], Some(1704123000))
      .commit_with_timestamp("(ui) Show the timeout in seconds", &[("settings.js", settings("30 // seconds").as_str())], Some(1704124800))
  }

  /// Stacked branches on, eleven branches, enough for the filter of the branch table: the stack `core` → `api` → `web`
  /// with `cli` forked off `core`, two issue branches, a prefix branch of an issue (`search`, IJPL-200) and standalone
  /// branches; "the API" is in the subject of `docs` too
  pub fn branch_filter() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("branch_filter")
      .branch_prefix("user-name")
      .config("branchdeck.stacks", "true")
      .commit_with_timestamp("Initial project setup", &[("README.md", "# Filter\n")], Some(1704117600))
      .commit_with_timestamp("(core) Add the core module", &[("core.js", "export function core() {}\n")], Some(1704119400))
      .commit_with_timestamp("(core/api) Add the REST API", &[("api.js", "export function api() {}\n")], Some(1704121200))
      .commit_with_timestamp("(api/web) Add the web client", &[("web.js", "export function web() {}\n")], Some(1704123000))
      .commit_with_timestamp("(core/cli) Add the command line", &[("cli.js", "export function cli() {}\n")], Some(1704124800))
      .commit_with_timestamp("(docs) Document the API", &[("docs.md", "# API\n")], Some(1704126600))
      .commit_with_timestamp("(ci) Run the tests on every push", &[("ci.yml", "on: push\n")], Some(1704128400))
      .commit_with_timestamp("IJPL-101: Speed up the indexing", &[("index.js", "export function index() {}\n")], Some(1704130200))
      .commit_with_timestamp("IJPL-102: Cache the parsed files", &[("cache.js", "export const cache = {}\n")], Some(1704132000))
      .commit_with_timestamp("(search) IJPL-200: Add fuzzy search", &[("search.js", "export function search() {}\n")], Some(1704133800))
      .commit_with_timestamp("(lint) Enable the strict rules", &[("lint.json", "{}\n")], Some(1704135600))
      .commit_with_timestamp("(deps) Update the dependencies", &[("package.json", "{}\n")], Some(1704137400))
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
  pub const fn empty_non_git() -> EmptyNonGitTemplate {
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

  pub const fn empty_git() -> EmptyGitTemplate {
    EmptyGitTemplate
  }

  /// A repository right after `git init`: no commits, so no baseline branch
  pub struct EmptyGitTemplate;

  impl EmptyGitTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      fs::create_dir_all(output_path)?;
      run_git(output_path, &["init", "--quiet", "--initial-branch=master"])?;
      disable_background_maintenance(output_path);
      run_git(output_path, &["config", "user.name", "Test User"])?;
      run_git(output_path, &["config", "user.email", "test@example.com"])?;
      run_git(output_path, &["config", "branchdeck.branchPrefix", "user-name"])?;
      Ok(())
    }
  }

  pub const fn bare() -> BareTemplate {
    BareTemplate
  }

  /// A bare repository: git data without a working tree
  pub struct BareTemplate;

  impl BareTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      fs::create_dir_all(output_path)?;
      run_git(output_path, &["init", "--quiet", "--bare", "--initial-branch=master"])
    }
  }

  fn run_git(dir: &Path, args: &[&str]) -> Result<()> {
    let output = git_command().args(args).current_dir(dir).output()?;
    anyhow::ensure!(output.status.success(), "git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr));
    Ok(())
  }

  /// Repository with commits and uncommitted changes for testing amend functionality
  pub struct AmendChangesTemplate;

  pub const fn amend_changes() -> AmendChangesTemplate {
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
      git_command().args(["add", "src/utils.js"]).current_dir(output_path).output()?;

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
    /// A local commit changes another word of the line that the upstream changed: git conflicts, the words resolve
    ResolvesByWords,
    /// A local commit adds the `WIDE_CONFLICT_FILES` files that each of 7 upstream commits changes: more files than a
    /// check reports, more upstream commits per file than the dialog lists
    WideConflict,
    /// The local commits rebase cleanly, but the file the upstream changed has uncommitted changes
    Blocked,
    /// A robot pushed `feature-auth` and the first of the two `search` commits: the upstream has them rebased, without
    /// the prefix, with their author and author time and the committer `Patronus`; `ui` stays local. The `search` commit
    /// names `JIRA-42`, which the `.idea/vcs.xml` of the issue links template links
    Landed,
  }

  /// A clone whose `master` tracks `origin/master` of an upstream that has moved on. The upstream is a bare repository
  /// at `.git/upstream.git`, the remote URL is relative, so a copy of the template fetches from its own upstream,
  /// offline. The upstream commits are already fetched, so a sync shows the update state at once.
  pub struct UpdateTemplate(UpdateScenario);

  pub const fn behind_upstream() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::BehindUpstream)
  }

  pub const fn update_conflict() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::Conflict)
  }

  pub const fn update_words() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::ResolvesByWords)
  }

  pub const fn update_conflict_wide() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::WideConflict)
  }

  /// The files of `update_conflict_wide`, `src/wide/file00.txt` and on
  pub const WIDE_CONFLICT_FILES: usize = 60;

  fn wide_files(content: &str) -> Vec<(String, String)> {
    (0..WIDE_CONFLICT_FILES).map(|i| (format!("src/wide/file{i:02}.txt"), content.to_string())).collect()
  }

  fn as_refs(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files.iter().map(|(path, content)| (path.as_str(), content.as_str())).collect()
  }

  pub const fn update_blocked() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::Blocked)
  }

  pub const fn landed_upstream() -> UpdateTemplate {
    UpdateTemplate(UpdateScenario::Landed)
  }

  pub const UPDATE_APP_FILE: &str = "src/app.txt";

  impl UpdateTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      let scenario = self.0;
      fs::create_dir_all(output_path)?;
      // `dates`: the author and the committer time
      let git = |args: &[&str], dates: Option<(i64, i64)>| -> Result<()> {
        let mut command = git_command();
        if let Some((author_time, committer_time)) = dates {
          command
            .env("GIT_AUTHOR_DATE", format!("{author_time} +0000"))
            .env("GIT_COMMITTER_DATE", format!("{committer_time} +0000"));
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
      // one `add` of every file of the commit: an `add` per file was 480 of the processes of `update_conflict_wide`
      let commit_as = |author: &str, message: &str, files: &[(&str, &str)], dates: (i64, i64)| -> Result<()> {
        let mut add = vec!["add", "--"];
        for (path, content) in files {
          write(path, content)?;
          add.push(path);
        }
        git(&add, None)?;
        git(&["commit", "-q", &format!("--author={author}"), "-m", message], Some(dates))
      };
      let commit = |message: &str, files: &[(&str, &str)], timestamp: i64| commit_as("Test User <test@example.com>", message, files, (timestamp, timestamp));

      git(&["init", "-q", "--initial-branch=master"], None)?;
      append_config(
        output_path,
        &[
          NO_BACKGROUND_MAINTENANCE[0],
          NO_BACKGROUND_MAINTENANCE[1],
          ("user.name", "Test User"),
          ("user.email", "test@example.com"),
          ("branchdeck.branchPrefix", "user-name"),
        ],
      );
      let mut initial_files = vec![
        ("README.md", "# Update Project\n"),
        (UPDATE_APP_FILE, "line 1\nline 2\nline 3\n"),
        ("src/config.txt", "debug=false\n"),
      ];
      if scenario == UpdateScenario::Landed {
        // the issue key of a landed commit is a link
        initial_files.push((".idea/vcs.xml", ISSUE_LINKS_VCS_XML));
      }
      commit("Initial commit", &initial_files, 1704117600)?;

      git(&["init", "-q", "--bare", "--initial-branch=master", BARE_ORIGIN], None)?;
      disable_background_maintenance(&output_path.join(BARE_ORIGIN));
      // what `remote add origin` and `branch --set-upstream-to=origin/master` write, appended
      append_config(
        output_path,
        &[
          ("remote.origin.url", BARE_ORIGIN),
          ("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"),
          ("branch.master.remote", "origin"),
          ("branch.master.merge", "refs/heads/master"),
        ],
      );
      git(&["push", "-q", "origin", "master"], None)?;

      // upstream commits, pushed by URL so the tracking ref stays until the fetch below
      git(&["checkout", "-q", "--detach"], None)?;
      // another author, as on a shared upstream; it reached the upstream 15 hours after it was written, so the conflict
      // dialog and the prompt show both times. Every time is before the fixed clock of the e2e pages (2024-01-02 10:00 UTC)
      commit_as(
        "Dana Lee <dana@example.com>",
        "Change line 2 upstream",
        &[(UPDATE_APP_FILE, "line 1\nline 2 upstream\nline 3\n")],
        (1704121200, 1704175200),
      )?;
      commit("Add the guide", &[("docs/guide.md", "# Guide\n")], 1704175500)?;
      if scenario == UpdateScenario::WideConflict {
        for round in 0..7 {
          let time = 1704175600 + round * 60;
          let files = wide_files(&format!("upstream {round}\n"));
          commit_as(
            "Dana Lee <dana@example.com>",
            &format!("Generate the wide files, round {round}"),
            &as_refs(&files),
            (time, time),
          )?;
        }
      }
      if scenario == UpdateScenario::Landed {
        // as a Safe Push robot pushes the local commits below, the morning after they were written
        for (message, path, content, author_time) in [
          ("Add login", "src/auth.txt", "login\n", 1704118500),
          ("Add index for JIRA-42", "src/search.txt", "index\n", 1704118600),
        ] {
          write(path, content)?;
          git(&["add", "--", path], None)?;
          let output = git_command()
            .args(["commit", "-q", "-m", message])
            .env("GIT_AUTHOR_DATE", format!("{author_time} +0000"))
            .env("GIT_COMMITTER_DATE", "1704175800 +0000")
            .env("GIT_COMMITTER_NAME", "Patronus")
            .env("GIT_COMMITTER_EMAIL", "patronus@example.com")
            .current_dir(output_path)
            .output()?;
          anyhow::ensure!(output.status.success(), "robot commit failed: {}", String::from_utf8_lossy(&output.stderr));
        }
      }
      git(&["push", "-q", BARE_ORIGIN, "HEAD:refs/heads/master"], None)?;
      git(&["checkout", "-q", "master"], None)?;

      commit("(feature-auth) Add login", &[("src/auth.txt", "login\n")], 1704118500)?;
      if scenario == UpdateScenario::Conflict {
        commit("(feature-auth) Change line 2", &[(UPDATE_APP_FILE, "line 1\nline 2 local\nline 3\n")], 1704118800)?;
      }
      if scenario == UpdateScenario::ResolvesByWords {
        commit("(feature-auth) Capitalize line 2", &[(UPDATE_APP_FILE, "line 1\nLine 2\nline 3\n")], 1704118800)?;
      }
      if scenario == UpdateScenario::WideConflict {
        commit("(feature-auth) Add the wide files", &as_refs(&wide_files("local\n")), 1704118800)?;
      }
      if scenario == UpdateScenario::Landed {
        commit("(search) Add index for JIRA-42", &[("src/search.txt", "index\n")], 1704118600)?;
        commit("(search) Add query", &[("src/search.txt", "index\nquery\n")], 1704118700)?;
        commit("(ui) Add button", &[("src/button.txt", "button\n")], 1704118800)?;
      }
      git(&["fetch", "-q", "origin"], None)?;

      match scenario {
        UpdateScenario::BehindUpstream => write("src/config.txt", "debug=true\n")?,
        UpdateScenario::Blocked => write(UPDATE_APP_FILE, "line 1\nline 2\nline 3\nlocal edit\n")?,
        UpdateScenario::Conflict | UpdateScenario::ResolvesByWords | UpdateScenario::WideConflict | UpdateScenario::Landed => {}
      }
      Ok(())
    }
  }

  /// Repository with archived branches (integrated, partially integrated, and not integrated)
  /// Implements its own build to create archived refs under user-name/archived/<date>/...
  pub struct ArchivedBranchesTemplate;

  pub const fn archived_branches() -> ArchivedBranchesTemplate {
    ArchivedBranchesTemplate
  }

  impl ArchivedBranchesTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      // Initialize repo
      fs::create_dir_all(output_path)?;
      git_command().args(["init", "--initial-branch=main"]).current_dir(output_path).output()?;
      disable_background_maintenance(output_path);
      git_command().args(["config", "user.name", "Test User"]).current_dir(output_path).output()?;
      git_command().args(["config", "user.email", "test@example.com"]).current_dir(output_path).output()?;
      git_command().args(["config", "branchdeck.branchPrefix", "user-name"]).current_dir(output_path).output()?;

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
        git_command().args(["add", "."]).current_dir(output_path).output()?;
        Ok(())
      };
      let git_commit = |message: &str, ts: i64| -> Result<()> {
        let date_str = format!("{ts} +0000");
        let mut cmd = git_command();
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

      // origin/main at HEAD (current state with all commits), and the remote that has it
      let current_head = String::from_utf8(git_command().args(["rev-parse", "HEAD"]).current_dir(output_path).output()?.stdout)?;
      let current_head = current_head.trim();
      git_command()
        .args(["update-ref", "refs/remotes/origin/main", current_head])
        .current_dir(output_path)
        .output()?;
      add_bare_origin(output_path, "main")?;

      // Create archived branches with their own histories, starting from the initial commit
      let date = "2025-01-11";

      // helper: checkout a detached state at initial commit
      let co_initial = || -> Result<()> {
        // get initial
        let init = String::from_utf8(git_command().args(["rev-list", "--max-parents=0", "HEAD"]).current_dir(output_path).output()?.stdout)?;
        let init = init.trim().to_string();
        git_command().args(["checkout", "-f", &init]).current_dir(output_path).output()?;
        Ok(())
      };
      let create_archived_from = |branch_name: &str, steps: &[(&str, &str, &str, i64)]| -> Result<()> {
        co_initial()?;
        // create a temp branch
        let tmp = format!("tmp-{branch_name}");
        git_command().args(["checkout", "-b", &tmp]).current_dir(output_path).output()?;
        for (path, content, message, ts) in steps {
          write_file(path, content)?;
          git_add_all()?;
          git_commit(message, *ts)?;
        }
        // Use the expected archived prefix as per E2E docs and snapshots
        let full_ref = format!("user-name/archived/{date}/{branch_name}");
        git_command().args(["branch", &full_ref, "HEAD"]).current_dir(output_path).output()?;
        // return to master
        git_command().args(["checkout", "-f", "main"]).current_dir(output_path).output()?;
        // delete temp branch
        git_command().args(["branch", "-D", &tmp]).current_dir(output_path).output()?;
        Ok(())
      };

      // feature-full archived (fully integrated): point archived ref to current main HEAD
      // This results in zero right-side commits relative to main
      let archived_full_ref = format!("user-name/archived/{date}/feature-full");
      git_command().args(["branch", &archived_full_ref, "HEAD"]).current_dir(output_path).output()?;

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
