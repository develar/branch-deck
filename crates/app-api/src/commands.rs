//! The commands of the app, each declared once, here (the grammar of an entry: `registry.rs`). The doc comment of an
//! entry is the one of its function in `src/lib/bindings.ts`; its body lives in a family module (`reads::…`,
//! `session::…`, `remote::…`, `app::…`). Every command of the app is an entry here: the app has no command of its own
//! (`command_scan_test.rs`). Every host is a callback of `for_each_command!`: `api_commands!` gives each command its
//! `Command` and the list [`COMMANDS`], the app its `#[tauri::command]` wrappers and their registration, which the
//! bindings are exported from in the order of this list (`src-tauri/src/api.rs`), the e2e server its routes
//! (`crates/test-server/src/invoke.rs`), and the page mock of the e2e server its table, `crates/app-api/commands.json`
//! (written from `COMMANDS` by `commands_json_test.rs`). A type of an entry is named by its full path, which resolves
//! in each host.

/// Calls the macro `$callback` with every command of the app, in the grammar of `registry.rs`
#[macro_export]
macro_rules! for_each_command {
  ($callback:path) => {
    $callback! {
      /// Validates that a repository path exists and is a git repository
      /// Returns empty string if valid, error message if invalid
      read json validate_repository_path(params: $crate::reads::repository::ValidateRepositoryPathParams) -> String = $crate::reads::repository::validate_repository_path;

      /// The prefix the branches of the repository are named with, with where it came from: its git config read now (the
      /// global config alone for an empty path), which every later command of the repository resolves with
      /// (`BranchPrefixes`), and the app's prefix of the settings. Only the Settings window asks it (General names the
      /// repository's own prefix and the global one by their source); the main window takes the prefix of the view of its
      /// session (`TableHeader.prefix`). An inaccessible repository is an error
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
      /// typed bindings cannot say, so it has its own handler (`raw_handler` of `src-tauri/src/api.rs`) and its own call in
      /// the view (`src/lib/diff/highlight/backend-syntax.ts`). From the blobs the diffs read, else one `cat-file --batch`
      read raw get_blob_syntax(params: file_diffs::model::GetBlobSyntaxParams) -> Vec<u8> = $crate::reads::blobs::get_blob_syntax;

      /// The typed diff of each changed file of a commit, a range or the working tree, for the diff views, through `events`:
      /// the files, then the content of each file as it is known, the files in view (`priority_paths`) first. Through the
      /// memory cache of the app and the `file_diff` store, else one list and one `cat-file --batch`
      read json get_file_diffs(params: file_diffs::model::GetFileDiffsParams) [channel events: file_diffs::model::FileDiffEvent] -> () = $crate::reads::diffs::get_file_diffs;

      /// The merge of each conflicting file of a cherry-pick, for the conflict view, through `events`: the files with the
      /// stage blob ids that `merge-tree` gives, then the content of each file as it is known, the files in view first
      read json get_file_merges(params: file_diffs::model::GetFileMergesParams) [channel events: file_diffs::model::FileMergeEvent] -> () = $crate::reads::diffs::get_file_merges;

      /// The changed files of `base..head` with their line counts, from one `diff-tree` of the two commits
      /// (`RangeFilesCache::read`), kept in memory
      read json get_range_files(params: domain_types::review_request::GetRangeParams) -> std::sync::Arc<git_ops::commit_files::CommitFiles> = $crate::reads::diffs::get_range_files;

      /// The declarations a name in a diff of a review may mean, the most likely first (`file_diffs::declarations`): by the
      /// scopes of its file, else in the files of the review, from the blobs the diffs read and the summaries of the store,
      /// else in the index of the repository (`accio`), waited for as long as it runs
      read window find_declaration(params: file_diffs::model::FindDeclarationParams) -> file_diffs::model::FoundDeclarations = $crate::reads::symbols::find_declaration;

      /// The usages of a name in a diff of a review (`file_diffs::usages`): in its file, in the files of the review from the
      /// blobs the diffs read, then outside the review in the index of the repository (`accio`), waited for as long as it runs
      read window find_usages(params: file_diffs::model::FindDeclarationParams) -> file_diffs::model::FoundUsages = $crate::reads::symbols::find_usages;

      /// A place outside the review at a revision (`file_diffs::outside_place`), a use the review plan left outside: its
      /// file as the view of a declaration outside the review opens it, at the line, the name selected
      read json get_outside_place(params: file_diffs::model::OutsidePlaceParams) -> Option<file_diffs::model::DeclarationTarget> = $crate::reads::symbols::get_outside_place;

      /// The plan of a review (spec/branch/review-plan.spec.md, Status: Experimental): what changed in a branch since it was
      /// marked reviewed, the public declarations the reviewed commits add, remove or change with their usages at the head
      /// from the index of the repository (`accio`), the facts of each file (`files`), the links between the files
      /// (`links`), the dependency changes of the build files (`dependencies`), the files that get the same edit
      /// (`repeated_edits`) and the files in dependency order
      /// (`review_plan_core`, as `bd review-plan`). A newer plan of the window or its close supersedes it: `canceled`
      read window get_review_plan(params: review_plan::ReviewPlanParams) -> review_plan::ReviewPlan = $crate::reads::review_plan::get_review_plan;

      /// Keeps the base and the tip a branch has now as its reviewed pair, which a later plan compares the branch with, and
      /// deletes the pairs of the branches that are gone, in one transaction with CAS (`mark_reviewed_core`, as `bd
      /// mark-reviewed`)
      app(ReviewedRefs) json mark_branch_reviewed(params: review_plan::mark_reviewed::MarkReviewedParams) -> review_plan::mark_reviewed::ReviewedTip = $crate::app::mark_reviewed::mark_branch_reviewed;

      /// Kills the symbol search of the calling window in flight, if any (Escape or Cancel while it searches)
      app(CanceledRead) window cancel_symbol_search() -> () = $crate::reads::symbols::cancel_symbol_search;

      /// The regroup after which the branches that conflict now apply cleanly, found by trial builds (objects and the object
      /// cache only, no repository lock), of the branches named with the prefix Rust resolves for the repository
      /// (`PrefixInputs`). One search at a time: a new one cancels the one before.
      read json find_conflict_fix(params: trial_check::conflict_fix::FindConflictFixParams) -> trial_check::conflict_fix::ConflictFix = $crate::reads::conflict_fix::find_conflict_fix;

      /// Stops the running search between two trials; it then reports `canceled`. A sync calls it before it starts.
      app(CanceledRead) now cancel_conflict_fix() = $crate::reads::conflict_fix::cancel_conflict_fix;

      /// What a typed branch name is and the name the branch gets (`domain_types::branch_names::check_branch_name`); the page
      /// words the state
      read plain validate_branch_name(params: domain_types::branch_names::ValidateBranchNameParams) -> domain_types::branch_names::BranchNameCheck = $crate::reads::grammar::validate_branch_name;

      /// The blocks of the body of `message`, the lines after its subject line (`sync_utils::commit_body::body_blocks`)
      read plain commit_body_blocks(params: domain_types::commit::CommitBodyParams) -> Vec<domain_types::commit_body::CommitBodyBlock> = $crate::reads::grammar::commit_body_blocks;

      /// The view of the repository's open session at the version of its last patch (a window that lost a patch asks it);
      /// null when the session is not open, so the window subscribes again
      read session get_snapshot(params: $crate::session::view::SessionParams) -> Option<domain_types::session::RepositoryView> = $crate::session::view::get_snapshot;

      /// Subscribes the calling window to the patches of the repository's session on `patches`, opening it: the answer brings
      /// the window from the view it holds (`have`: the seed of the start, `window.__BD_MODEL__`; null for none) to the view at
      /// the subscription (the view, the patch from the seed, or nothing when it holds that view), the channel every patch
      /// after it until `unsubscribe_view`, the window's close or its next page load. While it lasts, the operations registry
      /// of the repository is watched (`Registry::subscribe`): the operations of `bd` show in the view, and a stamp of `bd`
      /// syncs the session. The session is the one of the prefix the git config names now (`AppContext::session_key_now`),
      /// which the view names (`TableHeader.prefix`): the page reads no prefix of its own
      read session(now, window) subscribe_view(params: domain_types::session::SubscribeViewParams) [channel patches: domain_types::session::ViewPatch, open] -> domain_types::session::ViewSubscription = $crate::session::view::subscribe_view;

      /// Ends a subscription of the calling window; the last one of a session makes it idle
      read now unsubscribe_view(params: $crate::session::view::UnsubscribeViewParams) = $crate::session::view::unsubscribe_view;

      /// A window of the repository gained or lost focus; a gained one reads the operations registry of the repository again
      /// and syncs when what a sync reads changed and Settings sync on focus, and answers when that ends. Null when nothing
      /// was asked (focus lost, the setting off, the session not open)
      app(SessionTickets) session(focused = params.focused) set_window_focus(params: $crate::session::view::WindowFocusParams) -> Option<domain_types::session::SyncAnswer> = $crate::session::view::set_window_focus;

      /// What the window of a subscription shows and needs of its session (`Demand`), in place of what it said before; the
      /// page sends it when it changes, at most four times a second, and again for a new subscription
      read now set_demand(params: domain_types::session::SetDemandParams) = $crate::session::view::set_demand;

      /// Asks the repository's session for a sync, opening it, and answers when the run that serves the request ended (never
      /// the follow-ups after it), with the version its last patch brought the view to; a failed or deferred sync answers its
      /// `ApiError` (`operation_in_progress`, `detached_head`)
      app(SessionTickets) session(reason = ?params.reason) sync(params: $crate::session::view::SyncParams) -> domain_types::session::SyncAnswer = $crate::session::view::sync;

      /// Asks the repository's session to read a fact again: the refresh button, or an action that needs it now (the robot a
      /// `/patronus` comment started). Answers once that read applied, with the version of the view after its patch; null when
      /// nothing was asked (the rules held it back, no session is open)
      read session(family = ?params.family, reason = ?params.reason, subject = ?params.subject) refresh_facts(params: domain_types::session::RefreshFactsParams) -> Option<domain_types::session::Version> = $crate::session::view::refresh_facts;

      /// The group `message` would put its commit into, with the stacked branches, the issue groups and the stack parents
      /// the table of the repository's session was grouped with (`Sessions::preview_group`): the "Move to…" form previews a
      /// typed name as `(<name>)`, the message editor a typed subject. Null for an unassigned commit and before the session
      /// is open
      read session preview_group(params: domain_types::commit::PreviewGroupParams) -> Option<domain_types::commit::CommitGroup> = $crate::session::view::preview_group;

      /// The plan of an intent over the facts the session of the repository holds: what a start of a Safe Push or a Dry Run
      /// does now (`operations::land::prepare`, the rule `bd safe-push` decides by), what the user accepts first, what refuses
      /// it, the facts it waits for and the profiles it offers; the restart or the cancel of a robot, the retarget of a pull
      /// request (`operations::engine::prepare`). A read: no lock, no effect. Null while the session is not open, before its
      /// Patronus view, for a subject its table lacks, a robot it offers no restart of, a pull request it doesn't list
      read session prepare(params: domain_types::operation::PrepareParams) -> Option<domain_types::operation::Plan> = $crate::session::operations::prepare;

      /// Runs an intent on the plan the page showed (`fingerprint`, with the confirmations the user `accepted`): the plan is
      /// prepared again and runs only on that fingerprint; its steps, the progress of its push and its outcome go to
      /// `events` (`operations::engine::execute`). The review fetch of a window is stopped when the window closes
      /// (`WindowFetches`)
      engine session execute(params: domain_types::operation::ExecuteParams) [channel events: domain_types::operation::OperationEvent] -> domain_types::operation::Outcome = $crate::session::operations::execute;

      /// Stops the operation `op` of this app: a wait for the repository lock ends as `canceled`, and no effect starts after
      /// it; a push that sends goes to its end. False for an operation that ended
      engine now cancel_operation(params: domain_types::operation::CancelOperationParams) -> bool = $crate::session::operations::cancel_operation;

      /// Adds a pull request to Reviews by its link, `#number` or number (`remote_facts::reviews::link_pull_request`): GitHub
      /// is asked for it, an open one is kept for the repository, and the session reads Reviews again with it. Answers the
      /// pull request, which the page opens in the review window
      app(SettingsWriter) session link_pull_request(params: domain_types::review_request::LinkPullRequestParams) -> domain_types::review_request::LinkedPullRequestAnswer = $crate::session::reviews::link_pull_request;

      /// Removes a pull request the user added by its link from Reviews, and the session reads Reviews again
      app(SettingsWriter) session(number = params.number) unlink_pull_request(params: domain_types::review_request::UnlinkPullRequestParams) -> () = $crate::session::reviews::unlink_pull_request;

      /// The text a new pull request of a virtual branch starts with (`bd describe`, else the commits it will show, with the
      /// summaries of their issues from the YouTrack the app is signed in to), and the user of the token it is created with
      read session(head = %params.head, base = %params.base, stack_ancestors = params.stack_ancestors.len()) get_new_pull_request(params: remote_facts::pull_request_create::GetNewPullRequestParams) -> domain_types::pull_request::NewPullRequest = $crate::remote::pull_requests::get_new_pull_request;

      /// The text the "Create pull request" dialog of the whole stack starts with (`read_new_stack_pull_request`): the
      /// descriptions of the stack branch and its members, else the subjects of their commits; and the user of the token,
      /// who is the assignee
      read session(root = %params.root, members = params.members.len()) get_new_stack_pull_request(params: remote_facts::stack_pull_request::GetNewStackPullRequestParams) -> domain_types::pull_request::NewPullRequest = $crate::remote::pull_requests::get_new_stack_pull_request;

      /// The users of the repository a review can be asked from whose login or name matches the query
      read session(query = %params.query) search_github_users(params: remote_facts::pull_request_create::SearchGitHubUsersParams) -> Vec<domain_types::pull_request::GitHubUser> = $crate::remote::pull_requests::search_github_users;

      /// The Safe Push statistics of the repository (`spec/repository/safe-push-statistics.spec.md`): everyone's robots into
      /// the target of the period, kept in the object cache, the progress and the numbers of the lists through `events`
      /// before the runs are read
      read session(baseline_branch = %params.baseline_branch, period = ?params.period, refresh = params.refresh) get_robot_stats(params: domain_types::robot_stats::GetRobotStatsParams) [channel events: domain_types::robot_stats::RobotStatsEvent] -> domain_types::robot_stats::RobotStatsResult = $crate::remote::patronus::get_robot_stats;

      /// Why the robot failed, with the commit map of its branch, for the prompt for an agent: fast from the details, or full
      /// with the failed builds, their tests and the log tails of the `teamcity` CLI
      read session(robot_id = %params.robot_id, full = params.full) get_robot_failure(params: domain_types::robot_failure::GetRobotFailureParams) -> domain_types::robot_failure::RobotFailureReport = $crate::remote::patronus::get_robot_failure;

      /// Which commits of the branches a running robot covers, by the hashes of the checked-out branch: the commit rows mark
      /// the commits added or changed since it started (`robot_subject_commit_map`). No commits for a stack without members.
      read json get_robot_commit_map(params: domain_types::robot_failure::GetRobotCommitMapParams) -> domain_types::robot_failure::RobotCommitMap = $crate::remote::patronus::get_robot_commit_map;

      /// The parts of a branch for its review window: each branch of its stack from the root, with its base, head and
      /// commits, from one `for-each-ref` and one `git log`; the `log` only for tips and a baseline the memo hasn't seen
      read json get_branch_review(params: sync_core::branch_review::GetBranchReviewParams) -> std::sync::Arc<sync_core::branch_review::BranchReview> = $crate::remote::branch_review::get_branch_review;

      /// Reads in the background what the review window of a branch asks for first: its parts (`get_branch_review`) and the
      /// files of `part` (one `diff-tree`, `RangeFilesCache::read`), so the window finds them in the caches. Answers at once;
      /// a failure is the window's to tell
      read json prefetch_branch_review(params: $crate::remote::branch_review::PrefetchBranchReviewParams) -> () = $crate::remote::branch_review::prefetch_branch_review;

      /// Two branch names for the selected commits from the user's Claude Code (`claude -p --model haiku`); a missing or
      /// logged-out `claude` and its failures are outcomes
      read json suggest_branch_names(params: domain_types::branch_name_suggestion::SuggestBranchNamesParams) -> domain_types::branch_name_suggestion::BranchNameSuggestionOutcome = $crate::remote::suggest::suggest_branch_names;

      /// The sign-ins of both accounts and where their tokens come from: for a window without a session (Settings); a window
      /// of a repository has them as the fact `auth` of its session
      read json get_auth_status() -> domain_types::sign_in::AuthStatus = $crate::remote::accounts::get_auth_status;

      /// The issue of `key` for the popover, from the YouTrack the token is for
      read json get_youtrack_issue(key: String) -> domain_types::youtrack::YouTrackIssueAnswer = $crate::remote::accounts::get_youtrack_issue;

      /// Signs in by `request`, or joins the sign-in of its provider that runs: its events on `events`, `done` last; a cancel
      /// ends it with the code `canceled`
      app(Accounts) json sign_in(request: domain_types::sign_in::SignIn) [channel events: domain_types::sign_in::SignInEvent] -> domain_types::sign_in::SignInDone = $crate::app::sign_in::sign_in;

      /// Ends the sign-in of `provider` that runs, if any: its calls answer `canceled`
      app(Accounts) json cancel_sign_in(provider: domain_types::sign_in::SignInProvider) -> () = $crate::app::sign_in::cancel_sign_in;

      /// Removes the token of the app's own sign-in to `account`, after a sign-in of it that runs ended as canceled. GitHub
      /// and the Hub keep the authorization until it is revoked there
      app(Accounts) json sign_out(account: domain_types::sign_in::Account) -> () = $crate::app::sign_in::sign_out;

      /// The log of Activity: the journals of the repositories of the recent projects, each once and each waited for within
      /// `ACTIVITY_READ_TIMEOUT` (one that is not read by then shows what it answered last, and tells every window
      /// `activity-changed` once it answered other entries), and the journal of the page; no git process
      read json read_activity() -> domain_types::activity::ActivityLog = $crate::app::activity::read_activity;

      /// Records what the page told of a repository in the journal of the page, and answers its entry
      app(PageJournal) json record_activity(params: domain_types::activity::PageActivity) -> domain_types::activity::PageEntry = $crate::app::activity::record_activity;

      /// Clear Log: the journal of the repository of the recent project and the entries of the page of it are forgotten
      app(PageJournal) json clear_activity(params: domain_types::activity::ClearActivityParams) -> () = $crate::app::activity::clear_activity;

      /// Applies the change of the calling window to the settings and sends it to every window (`settings_changed`, with the
      /// window's label as its origin); answers the change, whose version the window has applied then. The file follows
      /// within the debounce of the writer. A value of a field Rust reads that is invalid is refused and changes nothing
      app(SettingsWriter) window set_setting(params: $crate::app::settings::SetSettingParams) -> domain_types::settings::SettingsChange = $crate::app::settings::set_setting;

      /// The changes the calling window gets that came after `since`, the version of the snapshot its page started from: a
      /// change made while the page loaded reached no listener. The main window's page asks once it listens (its init script
      /// has the snapshot of the app's start, a reload too); a sub-window's gets them with its load (`sub_window_loaded`)
      read window get_settings_changes(since: u32) -> Vec<domain_types::settings::SettingsChange> = $crate::app::settings::get_settings_changes;

      /// Writes one key to the local config of the repository, or to the global config of the user, and answers the settings
      /// read again. A write of the branch prefix keeps the git prefix the read found for the repository's later commands
      /// (`BranchPrefixes`); a global one forgets the prefix of every other repository, which may take it
      app(SettingsWriter) json set_repository_config(params: $crate::app::repository_config::SetRepositoryConfigParams) -> sync_core::repository_config::RepositoryConfig = $crate::app::repository_config::set_repository_config;

      /// Opens a sub-window, or brings the existing one of `window_id` to the front with new data. A new one is built hidden
      /// on the monitor of the main window (the window state plugin restores where it was since) and shows once its page
      /// settled its content (`sub_window_ready`). A new window starts from the settings as the writer has them now.
      app(TheApp) window open_sub_window(params: $crate::app::windows::OpenSubWindowParams) -> () = $crate::app::windows::open_sub_window;

      /// The page of the calling window loaded and listens to the settings: a sub-window gets the data of an open that came
      /// while it loaded and the changes of the settings after the snapshot its window was built with. The window shows once
      /// the page settled its content, at most `DRAW_FALLBACK` from now when the page draws no shell (`sub_window_drawn`)
      app(TheApp) window sub_window_loaded() -> $crate::app::windows::SubWindowLoaded = $crate::app::windows::sub_window_loaded;

      /// The loaded page of the calling window committed its shell, the title row (the main window's with its loading table):
      /// the window shows once the page settled its content, at most the content budget of the page from now
      /// (`CONTENT_BUDGET`, the main window's `MAIN_CONTENT_BUDGET`), so a slow page shows its shell, never a bare canvas
      app(TheApp) window sub_window_drawn() -> () = $crate::app::windows::sub_window_drawn;

      /// The page of the calling window settled the content of the open `open_id` (the main window: drew its first rows or
      /// the state without rows): the window shows if that is the latest open and the window is still hidden, once its
      /// webview presented that content (`show_window`)
      app(TheApp) window sub_window_ready(open_id: u32) -> () = $crate::app::windows::sub_window_ready;

      /// The window manager's menu of the window (Move, Resize, Always on Top, Close…), as a right click on a native title bar
      /// opens it: the page calls it for a right click on the empty parts of its title bar (`WindowTitleBar`). A sync command,
      /// so it runs on the main thread, where GDK and Win32 menus live.
      app(TheApp) main show_window_menu() -> () = $crate::app::windows::show_window_menu;

      /// Opens a native file dialog to browse for a git repository
      app(TheApp) json browse_repository() -> sync_core::repository_validation::BrowseResult = $crate::app::desktop::browse_repository;

      /// Checks Auto-sync on Focus of the native menu as the setting is, whichever window changed it: the main window calls it
      /// for every value of `appSettings.autoSyncOnFocus` (`useMenuSync`). Without a native menu (a custom title bar on
      /// Windows and Linux, whose main menu reads the setting itself) there is nothing to check.
      app(TheApp) main set_auto_sync_menu_checked(checked: bool) -> () = $crate::app::desktop::set_auto_sync_menu_checked;

      /// Restarts the app with "Use the system title bar" at `system_title_bar`: the title bar is decided once per process
      /// (`init_title_bar`). The value goes through the writer of the settings, which saves it now, before the restart: the
      /// new process reads the file.
      app(TheApp) main restart_app(system_title_bar: bool) -> () = $crate::app::desktop::restart_app;

      app(TheApp) json check_for_updates() -> domain_types::update::UpdateInfo = $crate::app::desktop::check_for_updates;

      read json get_update_status() -> domain_types::update::UpdateInfo = $crate::app::desktop::get_update_status;

      app(TheApp) json install_update() -> () = $crate::app::desktop::install_update;

      app(TheApp) json install_agent_cli() -> $crate::app::agent_cli::AgentCliInstall = $crate::app::agent_cli::install_agent_cli;

      /// The bd command of the prompts the app copies. Writes nothing and needs no install, so the bd of a prompt is always
      /// the version of the app that wrote it; an error when the app can't tell its binary (no `bd.exe` next to it)
      read json agent_cli_command(repository_path: String) -> $crate::app::agent_cli::AgentCliCommand = $crate::app::agent_cli::agent_cli_command;

      /// The agents of the IDE with Air that has `params.project` open, with their models and efforts (Start in Air); no IDE
      /// and a refusal of the IDE are answers. The IDE may ask the user first: the answer can take up to 120 s
      read json air_launch_targets(params: $crate::app::air_launch::AirTargetsParams) -> $crate::app::air_launch::AirTargetsAnswer = $crate::app::air_launch::air_launch_targets;

      /// Starts an agent session in the IDE with Air that has `params.project` open, or opens a draft of it (Start in Air);
      /// no IDE and a refusal of the IDE are answers. The IDE may ask the user first: the answer can take up to 120 s
      app(AgentLaunch) json air_launch(params: air_launch::protocol::LaunchRequest) -> $crate::app::air_launch::AirLaunchAnswer = $crate::app::air_launch::air_launch;

      /// The skill files of `bd install` that exist, without writing anything
      read now agent_cli_status() -> $crate::app::agent_cli::AgentCliStatus = $crate::app::agent_cli::agent_cli_status;

      /// The status of every tool, each read in its own threads, with the newest releases (GitHub, once a day); every window
      /// reads the Setup dot again
      read json cli_tools_status() -> Vec<domain_types::cli_tools::CliToolStatus> = $crate::app::cli_tools::cli_tools_status;

      /// Runs the action of a row (install, with the agent skill when `with_skill`; update; the skill; the sign-in of
      /// `teamcity` to `server`), each line of its output to `progress`; a second action of the same tool while one runs is
      /// refused
      app(Tools) json run_cli_tool_action(params: $crate::app::cli_tools::RunCliToolActionParams) [channel progress: $crate::app::cli_tools::CliToolProgress] -> $crate::app::cli_tools::CliToolActionDone = $crate::app::cli_tools::run_cli_tool_action;

      /// Ends the action of `tool` that runs, with its whole process group
      app(Tools) now cancel_cli_tool_action(tool: domain_types::cli_tools::CliTool) = $crate::app::cli_tools::cancel_cli_tool_action;

      /// What waits for the user (the Setup dot): the downloaded update of the app, the tools' updates and expiring sign-ins
      /// from their last statuses (read now when none is known), the app's YouTrack sign-in; without what `skips` hides
      read json get_setup_items(skips: Vec<domain_types::setup::SetupSkip>) -> Vec<domain_types::setup::SetupEntry> = $crate::app::cli_tools::get_setup_items;
    }
  };
}

crate::for_each_command!(crate::api_commands);
