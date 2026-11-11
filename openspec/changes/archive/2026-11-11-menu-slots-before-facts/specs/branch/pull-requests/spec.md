## MODIFIED Requirements

### Requirement: Reading pull requests: The repository is the configured one, origin's or the Space mirror

The repository MUST be `git config branchdeck.githubRepository` (`owner/name`), if set; else the one `origin` pushes to
(`git remote get-url --push origin`), as the virtual branches are pushed to `origin`; else, of an `origin` on Space, the
GitHub mirror that the Space settings committed in the repository name (`mirror.url` of
`.space/repository-settings.json`, read from `HEAD` with one `git cat-file`, never from the working tree). The sync
reads it with the remotes (`LandingRemote::github`), so the menu offers its pull requests from the first view: no
process for a configured repository or an `origin` elsewhere, the one `cat-file` for an `origin` on Space.

#### Scenario: The configured repository wins over origin
- **GIVEN** `git config branchdeck.githubRepository` is set (Settings → Landing, the tab of the repository, GitHub
  repository, which shows the detected one as its placeholder, [Settings](../../repository/settings/spec.md#landing))
- **WHEN** the repository is resolved
- **THEN** it is the configured one, not the one `origin` pushes to
  [@test] crates/sync-core/src/github_repository_test.rs (`the_configured_github_repository_wins_over_origin`)

#### Scenario: The repository origin pushes to
- **GIVEN** no configured repository
- **WHEN** the pull requests are listed
- **THEN** they are those of the repository `origin` pushes to
  [@test] crates/remote-facts/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)

#### Scenario: The GitHub mirror of the committed Space settings
- **GIVEN** no configured repository and `origin` on Space
- **WHEN** `.space/repository-settings.json` of `HEAD` names a GitHub mirror in `mirror.url`
- **THEN** the repository is that mirror, read with one `git cat-file`, never from the working tree
- **AND** a mirror that is not on GitHub names no repository, and the settings of an `origin` elsewhere are not read
  [@test] crates/sync-core/src/github_repository_test.rs (`reads_the_mirror_from_the_committed_space_settings`, `a_space_mirror_must_be_on_github`)

#### Scenario: The sync reads it with the remotes
- **WHEN** a sync reads the remotes
- **THEN** the GitHub repository is in the view from its first frame (`LandingRemote::github`), with no process for a configured repository or an `origin` elsewhere, one `cat-file` for an `origin` on Space
  [@test] crates/sync-core/src/repo_facts_test.rs (`the_landing_remote_is_read_with_the_facts_of_a_sync`)
  [@test] crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_a_space_origin_reads_its_github_mirror`)
