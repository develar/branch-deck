//! The commands of the app, each declared once, here (the grammar of an entry: `registry.rs`). The doc comment of an
//! entry is the one of its function in `src/lib/bindings.ts`; its body lives in a family module (`reads::…`). Every
//! host is a callback of `for_each_command!`: `api_commands!` gives each command its `Command` and the list
//! [`COMMANDS`], the app its `#[tauri::command]` wrappers (`src-tauri/src/api.rs`), the e2e server its routes
//! (`crates/test-server/src/invoke.rs`). A type of an entry is named by its full path, which resolves in each host.

/// Calls the macro `$callback` with every command of the app, in the grammar of `registry.rs`
#[macro_export]
macro_rules! for_each_command {
  ($callback:path) => {
    $callback! {
      /// Validates that a repository path exists and is a git repository
      /// Returns empty string if valid, error message if invalid
      read json validate_repository_path(params: $crate::reads::repository::ValidateRepositoryPathParams) -> String = $crate::reads::repository::validate_repository_path;

      /// Git, the git identity and a branch prefix suggestion, checked when the window opens and on every repository change
      read json check_git_setup(params: $crate::reads::repository::GetBranchPrefixParams) -> sync_core::git_setup::GitSetup = $crate::reads::repository::check_git_setup;

      /// The prefix the branches of the repository are named with, with where it came from: its git config read now (the
      /// global config alone for an empty path), which every later command of the repository resolves with
      /// (`BranchPrefixes`), and the app's prefix of the settings. The page shows the names with it, on open, on focus and
      /// after a change of either prefix; an inaccessible repository is an error
      read json resolve_branch_prefix(params: $crate::reads::repository::GetBranchPrefixParams) -> domain_types::settings::ResolvedBranchPrefix = $crate::reads::repository::resolve_branch_prefix;

      /// The `branchdeck.*` settings of the repository with their scopes, and the detected GitHub repository
      read json get_repository_config(params: $crate::reads::repository::GetRepositoryConfigParams) -> sync_core::repository_config::RepositoryConfig = $crate::reads::repository::get_repository_config;

      /// Get uncommitted changes with only file metadata (no content or diffs)
      read json get_uncommitted_changes(params: sync_core::uncommitted_changes::GetUncommittedChangesParams) -> sync_core::uncommitted_changes::UncommittedChangesResult = $crate::reads::commits::get_uncommitted_changes;

      /// The changed files of a commit with their line counts, from the object cache or one `diff-tree`
      read json get_commit_files(params: git_ops::commit_files::GetCommitFilesParams) -> git_ops::commit_files::CommitFiles = $crate::reads::commits::get_commit_files;

      /// The changed files of many commits (in the order of the ids), with one `diff-tree` for the ids the cache misses; the
      /// meta line of every list. The first view of a commit then needs no `diff-tree` (`get_file_diffs` reads the list here)
      read json get_commits_files(params: git_ops::commit_files::GetCommitsFilesParams) -> Vec<git_ops::commit_files::CommitFiles> = $crate::reads::commits::get_commits_files;

      /// The `fixup!`, `squash!` and `amend!` commits ahead of the baseline, grouped by the commit each one is squashed into
      read json get_autosquash_plan(params: history_rewrite::autosquash::GetAutosquashPlanParams) -> history_rewrite::autosquash::AutosquashPlan = $crate::reads::commits::get_autosquash_plan;

      /// What a move of commits would do, as the "Move to…" form previews it while the user types: the issues that go by
      /// their key and the commits that get a prefix, nothing written, no lock (`preview_move_core`). The move itself is the
      /// rewrite `Move` of `execute`
      read json preview_move(params: history_rewrite::move_commits::MovePreviewParams) -> domain_types::rewrite::MoveCommitsResult = $crate::reads::commits::preview_move;

      /// Whether a stacked branch builds on the baseline without its parent: a trial build of it as a root, objects only,
      /// through the object cache of the repository.
      read json get_stack_independence(params: trial_check::check::StackIndependenceParams) -> trial_check::check::StackIndependence = $crate::reads::commits::get_stack_independence;

      read json get_archived_branch_commits(repository_path: String, branch_name: String) -> Vec<domain_types::commit::Commit> = $crate::reads::commits::get_archived_branch_commits;

      read json get_unapplied_branch_commits(repository_path: String, branch_name: String) -> Vec<domain_types::commit::Commit> = $crate::reads::commits::get_unapplied_branch_commits;

      /// The code owners of paths by the `OWNERSHIP` files of the baseline, from one `cat-file --batch`; a path without an
      /// owner is left out
      read json get_file_owners(params: git_ops::ownership::GetFileOwnersParams) -> Vec<git_ops::ownership::FileOwner> = $crate::reads::commits::get_file_owners;

      /// The texts of blobs by id, for the diff views: mostly from the blobs that `get_file_diffs` read, else one
      /// `cat-file --batch`; a file of the working tree by its path, when its bytes still have the id
      read json get_blob_texts(params: file_diffs::model::GetBlobTextsParams) -> Vec<file_diffs::model::BlobText> = $crate::reads::blobs::get_blob_texts;

      /// The tree-sitter highlighting of the lines a view asks for of one blob (`file_diffs::syntax`): raw bytes, which the
      /// typed bindings cannot say, so it has its own handler (`raw_invoke_handler` in `lib.rs`) and its own call in the view
      /// (`src/lib/diff/highlight/backend-syntax.ts`). From the blobs the diffs read, else one `cat-file --batch`
      read raw get_blob_syntax(params: file_diffs::model::GetBlobSyntaxParams) -> Vec<u8> = $crate::reads::blobs::get_blob_syntax;

      /// What a typed branch name is and the name the branch gets (`domain_types::branch_names::check_branch_name`); the page
      /// words the state
      read plain validate_branch_name(params: domain_types::branch_names::ValidateBranchNameParams) -> domain_types::branch_names::BranchNameCheck = $crate::reads::grammar::validate_branch_name;

      /// The blocks of the body of `message`, the lines after its subject line (`sync_utils::commit_body::body_blocks`)
      read plain commit_body_blocks(params: domain_types::commit::CommitBodyParams) -> Vec<domain_types::commit_body::CommitBodyBlock> = $crate::reads::grammar::commit_body_blocks;
    }
  };
}

crate::for_each_command!(crate::api_commands);
