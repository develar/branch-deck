# Tasks

- [x] 1.1 `LandingRemote::patronus` and `LandingRemote::github` (`LocalGitHub`), read by the sync with the remotes; the
  Space settings only of an `origin` on Space; `SyncOptions::space` of the E2E server.
- [x] 1.2 The rules decide the presence of a slot on them: `PatronusStanding::of_remote`, `Listing::of_remote`,
  `GitHubFacts::of_remote`, the merge gate, `patronus_lands` and `github_repository_known` in `domain-policy`; Create
  Stack Pull Request… waits for the listing.
- [x] 1.3 "Create Pull Request…" before "Open Pull Request #N" on every surface (`pullRequestItems`).
- [x] 1.4 The table tests per remote kind, the scenario cases, the process budget of a sync of a Space `origin`.
- [x] 1.5 E2E tests: Space with Patronus held, GitHub with the listing held (Merge and Create → Open #7 in their
  places), a mirror (no Merge ever), a local remote (no slot from the first view); the pointer ones in WebKit too.
