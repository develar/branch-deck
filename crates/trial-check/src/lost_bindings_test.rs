use crate::lost_bindings::{BindingKind, LostName, ReplayLostBindings, ReplayRanges, lost_in, replay_lost_bindings};
use code_symbols::extract::summarize;
use code_symbols::language::Language;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, counting};

fn lost(language: Language, path: &str, before: &str, after: &str) -> Vec<LostName> {
  lost_in(&summarize(language, path, before.as_bytes()), &summarize(language, path, after.as_bytes()))
}

fn kotlin(before: &str, after: &str) -> Vec<LostName> {
  lost(Language::Kotlin, "src/Launch.kt", before, after)
}

fn names(lost: &[LostName]) -> Vec<(&str, BindingKind)> {
  lost.iter().map(|it| (it.name.as_str(), it.kind)).collect()
}

/// The file of the Safe Push that failed on 2026-11-20 (robot 1137287182), as an update replayed IJAI-6702 onto
/// IJAI-4983: the parent (the baseline) binds `backend` and `rethrowControlFlowException` and uses them in
/// `startUnsavedSessionAsNew`; the replayed commit drops the import and the property, which only the function it
/// removed used before, and keeps the new uses
const INCIDENT_BEFORE: &str = "package com.intellij.air.backend.claude.threads

import com.intellij.air.backend.prompt.api.hostOnWriterHost
import com.intellij.diagnostic.rethrowControlFlowException

internal class ClaudePromptLaunchSupport(
  /** The live registry of Claude, see [refuseResumeBesideDaemonJob]. */
  private val backend: () -> ClaudeThreadBackend = ::sharedClaudeThreadBackend,
) {
  override suspend fun hostLaunch(request: AgentHostLaunchRequest): AgentHostLaunchOutcome? {
    if (request is AgentHostLaunchRequest.Resume) refuseResumeBesideDaemonJob(request)
    return hostOnWriterHost(startUnsavedSessionAsNew(request))
  }

  private suspend fun refuseResumeBesideDaemonJob(request: AgentHostLaunchRequest.Resume) {
    val record = try {
      backend().liveSession(request.projectDirectory, request.sessionId)
    }
    catch (t: Throwable) {
      rethrowControlFlowException(t)
      null
    }
  }

  private suspend fun startUnsavedSessionAsNew(request: AgentHostLaunchRequest): AgentHostLaunchRequest {
    val hasTranscript = try {
      backend().hasThreadTranscript(request.projectDirectory, request.sessionId)
    }
    catch (t: Throwable) {
      rethrowControlFlowException(t)
      null
    }
    return request
  }
}
";

/// The replayed commit of the incident: `INCIDENT_BEFORE` without the import, the property and the function that was
/// their last use
fn incident_after() -> String {
  INCIDENT_BEFORE
    .replace("import com.intellij.diagnostic.rethrowControlFlowException\n", "")
    .replace("  /** The live registry of Claude, see [refuseResumeBesideDaemonJob]. */\n  private val backend: () -> ClaudeThreadBackend = ::sharedClaudeThreadBackend,\n", "")
    .replace("    if (request is AgentHostLaunchRequest.Resume) refuseResumeBesideDaemonJob(request)\n", "")
    .replace(
      "  private suspend fun refuseResumeBesideDaemonJob(request: AgentHostLaunchRequest.Resume) {\n    val record = try {\n      backend().liveSession(request.projectDirectory, request.sessionId)\n    }\n    catch (t: Throwable) {\n      rethrowControlFlowException(t)\n      null\n    }\n  }\n\n",
      "",
    )
}

#[test]
fn the_replayed_commit_of_the_incident_loses_the_property_and_the_import() {
  let lost = kotlin(INCIDENT_BEFORE, &incident_after());
  assert_eq!(
    names(&lost),
    vec![("backend", BindingKind::Declaration), ("rethrowControlFlowException", BindingKind::Import)]
  );
  let backend = &lost[0];
  assert_eq!((backend.binding_line, backend.use_line), (8, 13));
  let import = &lost[1];
  assert_eq!((import.binding_line, import.use_line), (4, 16));
}

#[test]
fn a_commit_that_removes_the_uses_too_loses_nothing() {
  let after = incident_after()
    .replace("      backend().hasThreadTranscript(request.projectDirectory, request.sessionId)\n", "      true\n")
    .replace("      rethrowControlFlowException(t)\n", "");
  assert_eq!(kotlin(INCIDENT_BEFORE, &after), Vec::new());
}

#[test]
fn a_name_still_imported_another_way_is_not_lost() {
  let alias = incident_after().replace(
    "import com.intellij.air.backend.prompt.api.hostOnWriterHost\n",
    "import com.intellij.air.backend.prompt.api.hostOnWriterHost\nimport com.intellij.util.rethrowControlFlowException\n",
  );
  assert_eq!(names(&kotlin(INCIDENT_BEFORE, &alias)), vec![("backend", BindingKind::Declaration)]);
  let star = incident_after().replace(
    "import com.intellij.air.backend.prompt.api.hostOnWriterHost\n",
    "import com.intellij.air.backend.prompt.api.hostOnWriterHost\nimport com.intellij.diagnostic.*\n",
  );
  assert_eq!(names(&kotlin(INCIDENT_BEFORE, &star)), vec![("backend", BindingKind::Declaration)]);
}

#[test]
fn a_declaration_that_stays_with_another_reach_is_not_lost() {
  let public = incident_after().replace(
    "internal class ClaudePromptLaunchSupport(\n",
    "internal class ClaudePromptLaunchSupport(\n  val backend: () -> ClaudeThreadBackend,\n",
  );
  assert_eq!(names(&kotlin(INCIDENT_BEFORE, &public)), vec![("rethrowControlFlowException", BindingKind::Import)]);
}

#[test]
fn a_qualified_use_or_an_extension_call_is_no_use_of_the_binding() {
  let before = "package a\n\nimport b.ext\n\nclass A(private val other: Other) {\n  fun f() = other.ext()\n}\n";
  let after = "package a\n\nclass A(other: Other) {\n  fun f(other: Other) = other.ext()\n}\n";
  assert_eq!(kotlin(before, after), Vec::new());
}

#[test]
fn a_local_of_the_same_name_takes_the_use() {
  let after = incident_after().replace(
    "    val hasTranscript = try {\n",
    "    val backend = { sharedClaudeThreadBackend() }\n    val hasTranscript = try {\n",
  );
  assert_eq!(names(&kotlin(INCIDENT_BEFORE, &after)), vec![("rethrowControlFlowException", BindingKind::Import)]);
}

#[test]
fn a_public_member_or_an_override_may_resolve_another_way() {
  let before = "package a\n\nclass A : B() {\n  fun helper() = 1\n  override fun base() = 2\n  fun f() = helper() + base()\n}\n";
  let after = "package a\n\nclass A : B() {\n  fun f() = helper() + base()\n}\n";
  assert_eq!(kotlin(before, after), Vec::new());
}

#[test]
fn an_import_of_a_default_or_the_own_package_binds_nothing_the_file_lacks() {
  let before = "package a\n\nimport kotlin.collections.List\nimport a.Helper\n\nfun f(items: List<Helper>) = items\n";
  let after = "package a\n\nfun f(items: List<Helper>) = items\n";
  assert_eq!(kotlin(before, after), Vec::new());
}

#[test]
fn a_type_whose_import_goes_is_lost() {
  let before = "package a\n\nimport b.Helper\n\nfun f(): Helper = Helper()\n";
  let after = "package a\n\nfun f(): Helper = Helper()\n";
  assert_eq!(names(&kotlin(before, after)), vec![("Helper", BindingKind::Import)]);
}

#[test]
fn java_loses_a_private_member_and_an_import_but_not_a_wider_member() {
  let before = "package a;\n\nimport b.Util;\n\nclass A extends B {\n  private int count;\n  protected int size;\n  int f() { return count + size + Util.ZERO; }\n}\n";
  let after = "package a;\n\nclass A extends B {\n  int f() { return count + size + Util.ZERO; }\n}\n";
  // `Util.ZERO` names `Util` bare as its qualifier's head: a use of the type
  let lost = lost(Language::Java, "src/A.java", before, after);
  assert_eq!(names(&lost), vec![("Util", BindingKind::Import), ("count", BindingKind::Declaration)]);
}

#[test]
fn a_degraded_side_tells_nothing() {
  let broken = format!("{}\n{}", incident_after(), "fun ((((( {{{{ ]]]] ".repeat(200));
  assert_eq!(kotlin(INCIDENT_BEFORE, &broken), Vec::new());
}

/// The commits from one `log --raw`, the summaries of both sides of each Kotlin or Java file they modify from one
/// `cat-file --batch`; a second read takes them from the object cache. A name the commit declares in another file it
/// changes moved there
#[test]
fn the_lost_bindings_of_commits_in_two_processes() {
  use crate::lost_bindings::find_lost_bindings;
  use test_utils::process_budget::cache_in;

  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "base",
    &[
      ("src/Launch.kt", INCIDENT_BEFORE),
      ("src/Other.kt", "package a\n\nimport b.Helper\n\nfun f(): Helper = Helper()\n"),
    ],
  );
  let incident = repo.create_commit_with_files("drops the property and the import", &[("src/Launch.kt", &incident_after())]);
  let moved = repo.create_commit_with_files(
    "moves Helper into the package",
    &[
      ("src/Other.kt", "package a\n\nfun f(): Helper = Helper()\n"),
      ("src/Helper.kt", "package a\n\nclass Helper\n"),
    ],
  );
  let dir = tempfile::tempdir().unwrap();
  let cache = cache_in(dir.path());
  let commits = vec![incident.clone(), moved];

  let git = counting();
  let lost = find_lost_bindings(&git, repo.path_str(), &cache, &commits).unwrap();
  assert_eq!(
    lost
      .iter()
      .map(|it| (it.commit.as_str(), it.file.as_str(), it.name.as_str(), it.use_line))
      .collect::<Vec<_>>(),
    vec![
      (incident.as_str(), "src/Launch.kt", "backend", 13),
      (incident.as_str(), "src/Launch.kt", "rethrowControlFlowException", 16)
    ]
  );
  assert_processes(&git, &[("cat-file", 1), ("log", 1)]);

  let git = counting();
  assert_eq!(find_lost_bindings(&git, repo.path_str(), &cache, &commits).unwrap(), lost);
  assert_processes(&git, &[("log", 1)]);
}

/// The rig behind the precision of the rule: the lost bindings of the commits of `LOST_BINDINGS_RANGE` (a `rev-list`
/// range, first parents) in `LOST_BINDINGS_REPO`, read only, with an object cache of its own; each finding printed with
/// whether a later commit of the range changed the file again. `cargo nextest run -p trial-check --run-ignored only -E
/// 'test(/lost_bindings_of_a_range/)' --no-capture`
#[test]
#[ignore = "a measurement on a real repository: LOST_BINDINGS_REPO, LOST_BINDINGS_RANGE"]
fn lost_bindings_of_a_range() {
  use crate::lost_bindings::find_lost_bindings;
  let (Ok(repo), Ok(range)) = (std::env::var("LOST_BINDINGS_REPO"), std::env::var("LOST_BINDINGS_RANGE")) else {
    return;
  };
  let git = GitCommandExecutor::new();
  let listed = git.cmd(&["rev-list", "--first-parent", "--no-merges", &range], &repo).text().unwrap();
  let commits: Vec<String> = listed.lines().map(str::to_string).collect();
  let dir = tempfile::tempdir().unwrap();
  let cache = test_utils::process_budget::cache_in(dir.path());
  let started = std::time::Instant::now();
  let mut found = Vec::new();
  for chunk in commits.chunks(500) {
    found.extend(find_lost_bindings(&git, &repo, &cache, chunk).unwrap());
  }
  println!("{} commits, {} findings in {:?}", commits.len(), found.len(), started.elapsed());
  for lost in &found {
    let subject = git.cmd(&["log", "-1", "--format=%h %s", &lost.commit], &repo).text().unwrap();
    println!(
      "{} | {}:{} {} ({:?}, bound at {})",
      subject.trim(),
      lost.file,
      lost.use_line,
      lost.name,
      lost.kind,
      lost.binding_line
    );
  }
}

/// The local commits of `clone` rebased onto its fetched upstream: HEAD before, the tip after, and the upstream
struct Rebased {
  original: String,
  tip: String,
  upstream_head: String,
}

fn rebase(clone: &TestRepo) -> Rebased {
  let original = clone.head();
  clone.git(&["fetch", "--quiet", "origin"]);
  clone.git(&["rebase", "--quiet", "origin/master"]);
  Rebased {
    original,
    tip: clone.head(),
    upstream_head: clone.git(&["rev-parse", "origin/master"]),
  }
}

/// `replay_lost_bindings` of `rebased` on `git`, without a cache, but the `kept` names
fn replay(git: &GitCommandExecutor, clone: &TestRepo, rebased: &Rebased, kept: &[(String, String)]) -> ReplayLostBindings {
  let ranges = ReplayRanges {
    tip: &rebased.tip,
    head: &rebased.original,
    upstream_head: &rebased.upstream_head,
    excluded: &[],
  };
  replay_lost_bindings(git, clone.path_str(), &ObjectCache::disabled(), ranges, kept).unwrap()
}

/// The copy of `TestRepo::upstream_and_clone_losing_bindings` (the incident) and its original from one `log`, the
/// summaries of the file the upstream changed too from one `cat-file --batch` (no cache): the copy loses what its
/// original doesn't, each use at the tip; a name the user kept is no finding
#[test]
fn a_replay_brings_the_lost_bindings_of_its_copy_in_two_processes() {
  let (_upstream, clone) = TestRepo::upstream_and_clone_losing_bindings(false);
  let rebased = rebase(&clone);
  let git = counting();
  let replayed = replay(&git, &clone, &rebased, &[]);
  let (original, tip) = (rebased.original.as_str(), rebased.tip.as_str());
  assert_eq!(
    replayed
      .found
      .iter()
      .map(|it| (it.original.as_str(), it.copy.as_str(), it.name.as_str(), it.kind, it.binding_line, it.use_line))
      .collect::<Vec<_>>(),
    vec![
      (original, tip, "backend", BindingKind::Declaration, 8, 11),
      (original, tip, "rethrow", BindingKind::Import, 4, 12)
    ]
  );
  assert_eq!(replayed.originals, vec![original.to_string()]);
  assert_processes(&git, &[("cat-file", 1), ("log", 1)]);

  let kept = [("src/Launch.kt".to_string(), "backend".to_string())];
  let names: Vec<String> = replay(&git, &clone, &rebased, &kept).found.into_iter().map(|it| it.name).collect();
  assert_eq!(names, ["rethrow"]);
}

/// A replay onto an upstream that changed no file of the local commits reads one `log` and no summary
#[test]
fn a_replay_that_changes_no_file_of_a_copy_reads_no_summary() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("src/Launch.kt", INCIDENT_BEFORE), ("other.txt", "other\n")]);
  upstream.create_commit("Upstream change", "other.txt", "changed\n");
  clone.create_commit("(launch) Drop the old path", "src/Launch.kt", &incident_after());
  let rebased = rebase(&clone);
  let git = counting();
  assert_eq!(replay(&git, &clone, &rebased, &[]).found, Vec::new());
  assert_processes(&git, &[("log", 1)]);
}

/// The edit of a lost-bindings round, by the rule of one file: the binding kept, the name stays lost no more
#[test]
fn an_edit_that_keeps_the_binding_loses_nothing() {
  use crate::lost_bindings::lost_names_in_edit;
  let after = incident_after();
  let names = |edited: &str| -> Vec<String> {
    lost_names_in_edit("src/Launch.kt", INCIDENT_BEFORE.as_bytes(), edited.as_bytes())
      .into_iter()
      .map(|it| it.name)
      .collect()
  };
  assert_eq!(names(&after), ["backend", "rethrowControlFlowException"]);
  let fixed = after.replace(
    "import com.intellij.air.backend.prompt.api.hostOnWriterHost\n",
    "import com.intellij.air.backend.prompt.api.hostOnWriterHost\nimport com.intellij.diagnostic.rethrowControlFlowException\n",
  );
  assert_eq!(names(&fixed), ["backend"]);
  assert_eq!(lost_names_in_edit("notes.txt", b"a", b"b"), Vec::new());
}
