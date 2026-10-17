# Changelog

## [0.18.0](https://github.com/develar/branch-deck/compare/v0.17.0...v0.18.0) (2026-10-17)


### Features

* **bd:** the skill allows every bd command, so Claude Code asks no permission for bd ([b23537a](https://github.com/develar/branch-deck/commit/b23537a355aff8977ce429fe93e0789078da52da))
* **conflict:** with the symbols on, the conflict fix moves by the edges by symbols, and `bd check` names missing declarations ([0f4970a](https://github.com/develar/branch-deck/commit/0f4970a0806c81b3b488ae404dacde7ee80d254c))
* **patronus:** a Safe Push without review from the pull request dialog, with a profile the user chooses ([a53e27a](https://github.com/develar/branch-deck/commit/a53e27ab1b855170a5ad19b466b6241fc4e6d7a3))
* **patronus:** one next step of a branch, rendered by every surface; the Safe Push form driven by readiness ([8b461c0](https://github.com/develar/branch-deck/commit/8b461c0429550f151f0c95dd205c85c90e3f0fc1))
* **patronus:** Patronus judges the approvals of the pull request; readiness shared by the app and bd ([3ad7559](https://github.com/develar/branch-deck/commit/3ad75595e6b68690c9cf4588d70b58c5f9076565))
* **preview:** the close of a preview focuses the row of what it showed last, so D goes on from there ([dcb5930](https://github.com/develar/branch-deck/commit/dcb5930ace5700b056cf1b012570863fc27bdef1))
* **pull-request:** Request Review… asks the suggested code owners on an open pull request; bd request-review ([9a0d4d3](https://github.com/develar/branch-deck/commit/9a0d4d3b1098de89f2232f55bf8c6896d4e8ecb9))
* **pull-request:** Request Review… is the next step of a group nobody asked, in the slot, the menu and the form ([49a376a](https://github.com/develar/branch-deck/commit/49a376ad9d8e89825ae6957f2492e3b28b625cce))
* **review:** an approval of the user's pull request is news with Safe Push… ([72b5a68](https://github.com/develar/branch-deck/commit/72b5a68e7720efa354f1863c3f27afe69d37283a))
* **review:** the Create pull request dialog opens in the review window it was asked in ([a8b1416](https://github.com/develar/branch-deck/commit/a8b1416bce707e044283de54ec22e00065ebdcbb))
* **settings:** a Settings window with a sidebar of pages instead of the modal dialog ([d6dc7ca](https://github.com/develar/branch-deck/commit/d6dc7ca5a4908f9b032a95bed34b161518b242c6))
* **settings:** one Settings button in the toolbar, and the prefix guide opens the field ([9603e10](https://github.com/develar/branch-deck/commit/9603e10031ed53b72fd5591da99a59a9eda1f16d))
* **settings:** read and write the branchdeck.* git config of a repository, the specific branch prefix first ([c344d6c](https://github.com/develar/branch-deck/commit/c344d6c8eaf1313fcb14683ed627ca84affdaa06))
* **stack:** with the symbols on, a stacked branch that applies alone names the names it uses of its parent ([7b6c80a](https://github.com/develar/branch-deck/commit/7b6c80a277c5de76cc55a00699a756c7671d4a65))
* **suggest:** with the symbols on, an edge by symbols orders the clusters and never joins them ([20a9661](https://github.com/develar/branch-deck/commit/20a9661130166b4bcf74edf29922bdc24d802d4a))
* **symbols:** on by default ([408a5bc](https://github.com/develar/branch-deck/commit/408a5bcdd64930ca5af0cc86ba6279f0bfe2b53a))
* **title-bar:** Linux draws its own title bar as IntelliJ does; one main menu for Windows and Linux ([9faf49a](https://github.com/develar/branch-deck/commit/9faf49aa669092f8523bea0eb9e99ad345fafdf4))


### Bug Fixes

* **check:** a remembered merge whose tree gc pruned is merged again ([c0e76f2](https://github.com/develar/branch-deck/commit/c0e76f20b4adc23e5712d25d6eb7baf2bdd2bd90))
* **menu:** reasons in descriptions, one name for Squash Fixups, a stable id per branch menu item ([08ba5b0](https://github.com/develar/branch-deck/commit/08ba5b072d7a2512c2b6dc96a31089ef8ae31446))
* **outgoing:** the overview says why a branch with a pull request is not outgoing ([a36a4f4](https://github.com/develar/branch-deck/commit/a36a4f4d21e8f82af43cf4c5014ccfcec3977446))
* **patronus:** robot toast steps are decided again at the click, the merge toast's Fetch only when it brings something ([a7fe83c](https://github.com/develar/branch-deck/commit/a7fe83c9b6d04cced1e3528d20330eb31f9a6fd8))
* **patronus:** S says why in a short title, the form leads with missing approvals, retarget only a former parent's base ([73528c2](https://github.com/develar/branch-deck/commit/73528c22f1c58105bc2d5a1a22770506b94cf065))
* **patronus:** the findings of the next-step review ([a365255](https://github.com/develar/branch-deck/commit/a365255a3be7ca8fe0cf13f26d4470fbc00ad318))
* **patronus:** the needs-review popover names the pull request, not a merge request ([2b9ab09](https://github.com/develar/branch-deck/commit/2b9ab091fa3b3b7cb58cbf6cf71978f0e3b82787))
* **patronus:** the Safe Push form pushes only by content, not by hash ([449feb1](https://github.com/develar/branch-deck/commit/449feb1fd8a68954a2d779514b80df40e5619fd1))
* **pull-request:** no Create Pull Request action that does nothing ([bc07d6c](https://github.com/develar/branch-deck/commit/bc07d6c28994a220569626d93eea1d97e64c3cff))
* **push:** one push in the branch menu, named and pushing as the row's push button ([c1a47d7](https://github.com/develar/branch-deck/commit/c1a47d7dba12a384ddd3475d37657ab977ed3902))
* **review-requests:** the row menu blocks Approve and Safe Push as the review does ([28b4eb3](https://github.com/develar/branch-deck/commit/28b4eb352b0bf7cadeb162bd2b49eebbd69d01b3))
* **review:** a review window shows when its first diffs are ready, not 3 s later ([e644cb2](https://github.com/develar/branch-deck/commit/e644cb24fd9128996f0451f8e3680583884211c4))
* **review:** the forms of the review take the focus on each open, also while they still close ([0cbff84](https://github.com/develar/branch-deck/commit/0cbff848ce350750c574247d21258c3e8298de27))
* **title-bar:** the Windows caption glyphs as escapes, not as private-use characters ([1156f02](https://github.com/develar/branch-deck/commit/1156f02636173f0cc8937886ef78f10ee0058602))


### Performance Improvements

* **check:** the memo of a check writes its merges in one transaction, not one per merge ([de70f8b](https://github.com/develar/branch-deck/commit/de70f8b5f7bef1647cda9a98d30f00cfb116209e))
* **dependencies:** only the declarations on the changed lines are compared, when a file keeps its namespace ([a1c54db](https://github.com/develar/branch-deck/commit/a1c54db9875f3e8ed3e372b79c1b3f7aeb71f312))
* **review:** a range reads its files with its patch in one git process, and a branch review is read ahead ([5292598](https://github.com/develar/branch-deck/commit/5292598bed12a5a4b8c05722031535ddafcd1711))
* **review:** a review window starts its reads with its page, beside the load of its route chunk ([cec93f7](https://github.com/develar/branch-deck/commit/cec93f7d1d6c42dfbb28e5b4b10715e164aebb31))
* **review:** patches from a backend cache, warmed for the listed commits ([750dffd](https://github.com/develar/branch-deck/commit/750dffd54c6ae0938674d6530e3eab98a3b544bd))
* **sync:** one merge and one commit process per branch, remote status without git for unchanged commits ([0f335de](https://github.com/develar/branch-deck/commit/0f335de952c26d65d8fc7c0264e59c80a5e2b48b))

## [0.17.0](https://github.com/develar/branch-deck/compare/v0.16.0...v0.17.0) (2026-10-17)


### Features

* **branch-review:** a branch is reviewed only in its window ([69c6083](https://github.com/develar/branch-deck/commit/69c608337b2ec916eb0e893db8431c308a45892a))
* **cache:** each store of the object cache has its own version, so a new value type keeps the other stores ([6fabec8](https://github.com/develar/branch-deck/commit/6fabec8325d03c1e90cdf81f89785e1e954bf2f7))
* **check:** `bd check --fix` finds the regroup after which the conflicting branches apply cleanly ([a9a8ca5](https://github.com/develar/branch-deck/commit/a9a8ca54c6cf03e501745e91c668522ba05d8c4a))
* **code-symbols:** declarations, imports and references of Kotlin, Java, Rust and TypeScript files on tree-sitter ([089d90a](https://github.com/develar/branch-deck/commit/089d90a91c21220c08e834c97e8557fa9198865a))
* **commit-review:** a commit is reviewed only in its window, Space keeps the quick preview ([7163aeb](https://github.com/develar/branch-deck/commit/7163aebeef05091701b00712a0821171ca29de3a))
* **commits:** `bd commits` names the edges by symbols: `via` and the names of each ([580995b](https://github.com/develar/branch-deck/commit/580995b594c42abb9ad907913a69aa49e8b095d2))
* **commits:** the cached changes of a commit carry the full blob ids before and after each file ([13825a2](https://github.com/develar/branch-deck/commit/13825a2a20b3895d0fb8bc9b0f6b86bc1e2efb4b))
* **conflict:** one view of the conflicting lines on the diff engine's unresolved file, instead of the Diff View and the 3-way tabs ([d3acd70](https://github.com/develar/branch-deck/commit/d3acd70cab3d0b3b4eea683024dd3ad1d3b03ffb))
* **conflict:** the conflict view offers the fix a trial build verified and applies it with Undo ([ff33c25](https://github.com/develar/branch-deck/commit/ff33c25e3157dade8ecafae0f508eac683c8ad37))
* **conflict:** the fix from the branch menu, the branch preview and the conflict windows ([81d5b73](https://github.com/develar/branch-deck/commit/81d5b73ecc45e8c653fcd9de68044a240821c73a))
* **conflict:** the missing commits name their branch, and the explanation says which branches changed the files first ([bae55ff](https://github.com/develar/branch-deck/commit/bae55ff73745d10b710d221706c94270ff490f11))
* **dependencies:** dependencies between commits by the symbols they declare and use, on tree-sitter ([fe7d6cf](https://github.com/develar/branch-deck/commit/fe7d6cf56953d530f65ccab257cbe8abbfd5655a))
* **dependencies:** the building blocks of the symbol scan: blobs as bytes, trees, module roots, the summary store ([44cd821](https://github.com/develar/branch-deck/commit/44cd8214aa45beb9be9918026a89f2c9ef4c7450))
* **pull-requests:** create the pull request in the app, with the suggested code owners as reviewers and the user as its assignee ([1c3ca1e](https://github.com/develar/branch-deck/commit/1c3ca1eb57b9f6467f3a8acf6959d8ee99b003d5))
* **review-requests:** a pull request is reviewed only in its window ([809a1de](https://github.com/develar/branch-deck/commit/809a1de13f42a02ecf97f1fb1f51206242175d4c))
* **review:** every window draws its title bar; one review sidebar; file rows give their width to the name ([c735001](https://github.com/develar/branch-deck/commit/c7350011e1d51ca9837c97e3091a9e794b5a2fb3))
* **update:** one progress from the click to the end, its steps in the tooltip ([58e41f5](https://github.com/develar/branch-deck/commit/58e41f5585857fa1940ca1e18d2ddc1d417421c1))
* **windows:** a sub-window shows once its page painted, and two quick opens build one window ([b248829](https://github.com/develar/branch-deck/commit/b248829e630e73289f2842c4a0c2494f6726afe5))


### Bug Fixes

* **auto-update:** perform_update_check is pub(super); clippy checks the auto-update feature ([452dbc3](https://github.com/develar/branch-deck/commit/452dbc3c7f49859c79d4291fc06dd5fdf9202add))
* **check:** a dangling-reference fix is offered only when a trial build proves it ([68ba37f](https://github.com/develar/branch-deck/commit/68ba37f23e67eb01b80d2cdb0024cebf0f56f5bc))
* **check:** the syntax trees take the comments, strings, locals and still-bound names out of the dangling references ([9583655](https://github.com/develar/branch-deck/commit/9583655705272dc01619aadce437427203bf621a))
* **commits:** Review Changes of a commit row's menu opens the review ([651af8e](https://github.com/develar/branch-deck/commit/651af8e6a0fea5d8c205cca78f18ec491e5a593d))
* **suggest:** a cycle of stacked clusters is broken inside it, not at a cluster that waits on it ([aa86b99](https://github.com/develar/branch-deck/commit/aa86b991669444c4c549bacf5e1681043f4f6dfb))
* **suggest:** a repair joins what the lines of a conflict need, and one mention takes no issue into a prefix branch ([18df05d](https://github.com/develar/branch-deck/commit/18df05dd049fca755bf9d4f800a1a2fb9c8e7e73))

## [0.16.0](https://github.com/develar/branch-deck/compare/v0.15.0...v0.16.0) (2026-10-16)


### Features

* **bd:** `--allow-append` accepts a HEAD with a peer's commits on top of `--expect-head` ([940379b](https://github.com/develar/branch-deck/commit/940379b5d147c2d0937c442e28bdbc26e1e32a81))
* **bd:** `bd check` names an inverted stack and the fixes that build ([00b89ac](https://github.com/develar/branch-deck/commit/00b89acbab99e070fc67c69145213cc4969aca1a))
* **bd:** `bd check`, `bd suggest` and `bd safe-push` find a removed declaration that the branch still uses ([ec33138](https://github.com/develar/branch-deck/commit/ec33138dfa8206cd12fecf88df0cf8d9f27e730e))
* **bd:** `bd move-hunk` moves a file's change between two commits ([fce478e](https://github.com/develar/branch-deck/commit/fce478e1d4f7677370dc6865f60e3d8194c61a91))
* **bd:** `bd sync` names the push each branch needs ([1619edd](https://github.com/develar/branch-deck/commit/1619eddd2f1e59f912ff52ff6e4f6c76b78ff39f))
* **bd:** a mutual dangling reference names its fix: move the commit, or a ready `bd move-hunk` ([2a08928](https://github.com/develar/branch-deck/commit/2a089284fc470dc87540ff7de7ae0dd31d23ef9a))
* **branch-review:** D reviews in the branch preview; W moves it to the window ([191fca9](https://github.com/develar/branch-deck/commit/191fca928e3c4e32746bde57db3dcd1debbbbc0c))
* **branch-review:** the commits of a part carry their message and author ([e5c1022](https://github.com/develar/branch-deck/commit/e5c1022adcb3d07bed4f5e0048d2af0c25a7080d))
* **branch:** one source of push and Safe Push blockers; the branch window on the link ([5146753](https://github.com/develar/branch-deck/commit/5146753a10a53381a1084ee08de81c8bf8b93040))
* **commit-review:** Review Changes and Open in Window for a commit; a commit-review window ([d457939](https://github.com/develar/branch-deck/commit/d45793916d66a818b63c2026873bab458e5d679d))
* **patronus:** Copy Prompt for Agent of a failed Safe Push, fast or with CI details ([ffb9a30](https://github.com/develar/branch-deck/commit/ffb9a30980c66738593903c87e730ea993194aff))
* **patronus:** the prompt and `bd robot` cover the whole stack a stacked branch's robot built ([6e57a9f](https://github.com/develar/branch-deck/commit/6e57a9fc582667f2acc1ef6372e06c9ec02be2c7))
* **patronus:** the Safe Push form and the move toast offer the unstack of a stacked branch that builds alone ([f0c5830](https://github.com/develar/branch-deck/commit/f0c5830398513f8ec284348453133f3f33d06bf5))
* **review-requests:** Changes are a range review; a commit opens Changes at it; D on a row ([bc80000](https://github.com/develar/branch-deck/commit/bc80000e3827d1fb435a79373b5fb9c9f2f25058))
* **review:** a range review, its commits beside the diffs ([4e06a41](https://github.com/develar/branch-deck/commit/4e06a418ea175e51aaf619965081efd467772ef7))
* **rows:** D and W on rows; Open in Window in the menus ([ed22189](https://github.com/develar/branch-deck/commit/ed221895859b499745be448d01359ac5f02a0c2a))
* **stack:** a big stack is an island of its own below the branch table ([3f621e9](https://github.com/develar/branch-deck/commit/3f621e9c96a8f2dd9391365eea4dc61c0dfcafab))
* **stack:** a side stack collapses into its root; a quiet one starts collapsed ([cff832f](https://github.com/develar/branch-deck/commit/cff832f9e8587ba5743a1648431cb87b0ccb152a))
* **stack:** Push Up to Here pushes a stack up to one of its commits ([a8af2f4](https://github.com/develar/branch-deck/commit/a8af2f4ad4c617a3833724d8b4b61e6daa897a5d))
* **stack:** tell whether a stacked branch builds alone, in the app command and in bd ([62dfa18](https://github.com/develar/branch-deck/commit/62dfa18c949a192981b2bdc954e8d63a9fddd181))
* **stack:** the island header is the Actions cell of a row, and nothing pushes into a running Safe Push ([97f49d6](https://github.com/develar/branch-deck/commit/97f49d65d425f630d7146f4af7b88bea8bc42dcf))
* **ui:** one help popover anatomy, the ⓘ right after the title it explains ([4cecb9c](https://github.com/develar/branch-deck/commit/4cecb9c15833cf48ad1f82b964926695ac9e6ad9))


### Bug Fixes

* **bd:** clippy on Windows: the helpers of unix-only tests are unix-only, the Windows stubs expect `unnecessary_wraps` ([173a929](https://github.com/develar/branch-deck/commit/173a9298f5afa55a98e5eb9e16fc1b0621955d29))
* **bd:** the dangling-reference check finds what breaks on the IntelliJ checkout, and in seconds ([e91e0a6](https://github.com/develar/branch-deck/commit/e91e0a6cea25c7c89690dee290b62f1dd6cc4338))
* **build:** one version in package.json, LF in every checkout ([b5b7253](https://github.com/develar/branch-deck/commit/b5b72535705c8ec6dbb3994bcc784821958d7832))
* **install:** say why the installed binary does not start; the AppImage job has the libraries of a desktop ([c1cee68](https://github.com/develar/branch-deck/commit/c1cee682639987630f8731228e1012e532309b7c))
* **push:** one indicator of a stack push and of the push of the Safe Push form ([aec2b47](https://github.com/develar/branch-deck/commit/aec2b4798ca345e59125de231528bdfa16053022))
* **sync:** a mapping note is checked against the commit before its copy is reused ([8e364eb](https://github.com/develar/branch-deck/commit/8e364ebd7b5636861793d095526929702d18e1e0))
* **sync:** one sync at a time, shown once by the spinner of Sync ([5251902](https://github.com/develar/branch-deck/commit/52519025264bafe14c30634bb58fa8ab1b4258a3))
* **update:** a short agent prompt for a large conflict ([2d47cad](https://github.com/develar/branch-deck/commit/2d47cad39a28675f80fe1aa24baf316dba018e68))


### Performance Improvements

* **update:** a large conflict costs one replay and a bounded list at every level ([0ef85b7](https://github.com/develar/branch-deck/commit/0ef85b70a5ff5c9e73c99e4cfa1c1f0ac823b494))

## [0.15.0](https://github.com/develar/branch-deck/compare/v0.14.0...v0.15.0) (2026-10-16)


### Features

* **github:** a GitHub API client to replace the gh CLI ([cc4cd40](https://github.com/develar/branch-deck/commit/cc4cd4080d60684402602ebdadb7cbb1af7f0419))
* **github:** pull requests, Your Turn and the writes through the GitHub API, no gh process ([2f916ec](https://github.com/develar/branch-deck/commit/2f916ecc73000a8398e8bb4ce9f5914b121f248b))
* **github:** say that an organization may have to approve the sign-in, and offer Request Access ([45e4afc](https://github.com/develar/branch-deck/commit/45e4afc0515816853239ed9376ec2a0923479d02))
* **github:** sign in to GitHub in the app, without gh ([c99e3bb](https://github.com/develar/branch-deck/commit/c99e3bb71577ee96bf3bb29e01928ddc95d6a286))
* **patronus:** a failed Safe Push is told once, with its next step, as a merge is ([70ab11f](https://github.com/develar/branch-deck/commit/70ab11f9c8d96c78e00d345be5d6361436a31bb4))
* **patronus:** a running robot whose required check failed is "Failing", told once with Cancel Safe Push ([092e260](https://github.com/develar/branch-deck/commit/092e26033384098b03ca4d4c719711f8296d6b81))
* **review:** a decision header for the review of a pull request, Approve the primary, and its own window ([d5d7bf9](https://github.com/develar/branch-deck/commit/d5d7bf979ef5160efe5ef47318506be9c1498aca))
* **row:** the Status cell says the robot, the Safe Push slot its next step ([ea2147b](https://github.com/develar/branch-deck/commit/ea2147bcd5cc374790dad87bab95003f1a330772))
* **sync:** Auto-sync on Focus syncs only what changed, and is on by default ([1e2e692](https://github.com/develar/branch-deck/commit/1e2e692f13639f7e5ced8936d2b85726bba047a5))
* **your-turn:** the reviews that wait for the user and the own pull requests handed back, one list ([82330a6](https://github.com/develar/branch-deck/commit/82330a64eec01047afde9c7b4fe0d4595bf373f3))


### Bug Fixes

* **github:** the hint's button says "Settings…" in a build without the sign-in ([d52ea42](https://github.com/develar/branch-deck/commit/d52ea42075b228f37e7c27b04db1d69633c2b795))
* **row:** a row shows the issue key once, not again in its title and its commits ([547050e](https://github.com/develar/branch-deck/commit/547050e72abb9944113eb9786e60b39d4b7dc80a))
* **stack:** the chip of a collapsed stack counts a failure as the header does ([a72a74b](https://github.com/develar/branch-deck/commit/a72a74bb9f3ca9e0e5c8f37c6863cf0b35f48b27))
* **sync:** no sync in the middle of a rebase; a sync of unchanged branches doesn't flash the rows ([2f1cc46](https://github.com/develar/branch-deck/commit/2f1cc46abd0b2509d34e56fa2afaea04e215a261))
* **update:** a sync follows every Update Project whose fetch finished, not only an applied one ([7d3f9cb](https://github.com/develar/branch-deck/commit/7d3f9cb3712799dbd0161ead39921a03cdf930a1))

## [0.14.0](https://github.com/develar/branch-deck/compare/v0.13.0...v0.14.0) (2026-10-16)


### Features

* **cli:** bd safe-push collects the approvals in the GitHub pull request, not a Space merge request ([e055f03](https://github.com/develar/branch-deck/commit/e055f031e46cfe3e43d6efe2600d993927876875))
* **cli:** bd safe-push starts a Patronus Safe Push or Dry Run with a merge profile ([d84950c](https://github.com/develar/branch-deck/commit/d84950cfe9d8d3fcd88e3d34331174cfff04bdb3))
* **patronus:** the estimate of a robot learns from robots of its merge profile ([c6023cd](https://github.com/develar/branch-deck/commit/c6023cdd34192eeffe113932d201c9bcdd6d19b8))
* **patronus:** the precise time left of a running robot, read only while its details are open ([22f39e1](https://github.com/develar/branch-deck/commit/22f39e1be298a7c936bbca64a3db77ce621dc352))
* **patronus:** the Safe Push form creates no Space merge request, the approvals go into the GitHub pull request ([87a0f4e](https://github.com/develar/branch-deck/commit/87a0f4ea168fffaa17c059eb8d23c9c451cab145))
* **patronus:** watch the robot of a reviewed pull request: polled, listed in the header, its merge told ([99980e0](https://github.com/develar/branch-deck/commit/99980e067add1098ee27a946636bf599f1b8368b))
* **review:** a branch review window with the stack and the files of each code owner ([e140c0a](https://github.com/develar/branch-deck/commit/e140c0a131152b36d1203cc0c6d222edbdb850d1))
* **review:** Approve and Safe Push a reviewed pull request with a plain /patronus merge ([74e3af2](https://github.com/develar/branch-deck/commit/74e3af205fe412f9b3235206239e5c4bbfc0330b))
* **review:** approve or request changes on a pull request from its review, in the preview and the review alike ([14b3f73](https://github.com/develar/branch-deck/commit/14b3f730544191a285023c33da0e44b6b693f376))
* **review:** review requests, the review fetch and range diffs ([275c3fc](https://github.com/develar/branch-deck/commit/275c3fcc1724ce8be3ebdf03ed22742e01638a90))
* **review:** test server: the head of a review from its branch, the keeps of the prunes ([6746f81](https://github.com/develar/branch-deck/commit/6746f815094f65d50fb7c15782b1be6d7fb9187c))
* **review:** the Branch Review window gets a footer with the branch actions ([35e1262](https://github.com/develar/branch-deck/commit/35e1262d2bb6006d35843cf0e88e177169e4f5ce))
* **review:** the commit preview of a review shows the whole message ([7a29654](https://github.com/develar/branch-deck/commit/7a2965447305bde677793a0907b55da49ca8eaaf))
* **review:** the contract of the review requests: types, command stubs, bindings ([4363e8b](https://github.com/develar/branch-deck/commit/4363e8bb6ffada9880da1136c8e3558b8c0efc88))
* **review:** the full message of each commit of a review range ([ff54dd4](https://github.com/develar/branch-deck/commit/ff54dd4676137e321e03f2e53180008f697fe791))
* **review:** the review commands and their test server routes ([6a0bfb6](https://github.com/develar/branch-deck/commit/6a0bfb6c388c49b24b2ed80925b087a2a011d1d2))
* **review:** Your Turn card, the review dialog, notifications and Settings ([39b8087](https://github.com/develar/branch-deck/commit/39b808724d0f8670c0285ce7795c728d08a7fd22))
* **settings:** the AI branch names have their switch in Settings ([ec25d19](https://github.com/develar/branch-deck/commit/ec25d1967a0f372e12aaba8ab146c9fed76da20e))
* **stack:** a stack reads as one block, full-width dividers only between the units ([78a281e](https://github.com/develar/branch-deck/commit/78a281e1c0732d573217fb9ab46d715b18b9ed48))
* **stack:** one size of a stack everywhere, "22 branches · 152 commits" ([9ac8437](https://github.com/develar/branch-deck/commit/9ac84371f9ed05a03ca956836581cccdfdfdae2c))
* **stack:** stack parents stack a branch without a history rewrite ([649a113](https://github.com/develar/branch-deck/commit/649a113e7290f503402fe432f7a6f2c20a489c58))
* **stack:** the graph highlights the path from the root to the hovered branch ([bb1dbf7](https://github.com/develar/branch-deck/commit/bb1dbf7cc2b56a0d8a99a15619a397f9858aba65))
* **stack:** the root row heads its stack, "22 branches", which collapses the stack into it ([925bace](https://github.com/develar/branch-deck/commit/925bace4ead61bdb8519b14adea947b3a606f920))


### Bug Fixes

* **archive:** archive before the rebuild, by namespace, never a checked-out branch ([7e77503](https://github.com/develar/branch-deck/commit/7e77503462cc913289cff88564e79cb129bba6bd))
* **e2e:** a reload starts from what the app wrote to its settings store ([826f0bd](https://github.com/develar/branch-deck/commit/826f0bd6835683af493b4228de6a372cac87ef30))
* **row:** the Safe Push slot holds still while a finished robot's details are open ([c9c809d](https://github.com/develar/branch-deck/commit/c9c809dbeccbc95e5f3991ff896bddb23dbe1978))
* **sync:** a remote status computed mid-push is dropped on arrival, not at the flush ([d43808c](https://github.com/develar/branch-deck/commit/d43808c0dcc42b87eef94d8028f96ee0b8420e23))


### Performance Improvements

* **frontend:** React Compiler compiles every component and hook ([a59baf7](https://github.com/develar/branch-deck/commit/a59baf73c3be1a9010ff25d045b3bfe8f1d95677))
* **frontend:** sub-windows load only their page, the Inter font is preloaded ([c09f7ab](https://github.com/develar/branch-deck/commit/c09f7ab2c4fcb8ece621103a69d6ebcff9f5de59))
* **sync:** one store update per batch of sync events, cached table structure ([ad5b2c7](https://github.com/develar/branch-deck/commit/ad5b2c71d216c882620d07aa98eb0cf463151c05))

## [0.13.0](https://github.com/develar/branch-deck/compare/v0.12.0...v0.13.0) (2026-10-15)


### Features

* a stacked branch's unpushed commits exclude its parent's ([c34e0f4](https://github.com/develar/branch-deck/commit/c34e0f4c8aba3efa45f2c1c89562cab1f1ce594b))
* add auto-sync on window focus ([fd18d51](https://github.com/develar/branch-deck/commit/fd18d51fa9dfab92ada0c6a4328b32320fe22921)), closes [#45](https://github.com/develar/branch-deck/issues/45)
* amend local changes to a branch ([2ceb6a2](https://github.com/develar/branch-deck/commit/2ceb6a24197f47ab80bde04323ae65c8b57c6ec9))
* amend local changes to a branch ([fd2ad77](https://github.com/develar/branch-deck/commit/fd2ad7759f41f5dba914e29dc5959964db7fd915))
* auto-update ([00ab5ef](https://github.com/develar/branch-deck/commit/00ab5efc56384520a83678c5264ea5682ec95659))
* autosquash matches fixups by stack leaf ([15fefea](https://github.com/develar/branch-deck/commit/15fefeae2d666eaa3c40e825d631643cb9928a58))
* bd agent CLI with read-only status, commits, schema and help agent ([ce833ce](https://github.com/develar/branch-deck/commit/ce833ce06d2924cdba4324cbeeddd76bce6e76a9))
* blocked branches offer no push anywhere, stacked ones name their base ([662b3bd](https://github.com/develar/branch-deck/commit/662b3bd7796e7f9f7025de2ce47a5a1e85f63541))
* branch name suggestions from Claude Code (Haiku) instead of the on-device model ([65e38b9](https://github.com/develar/branch-deck/commit/65e38b98edc5b7aff51ba4f15037c80cf860452d))
* branch parallel processing, sanitize branch name ([8821954](https://github.com/develar/branch-deck/commit/8821954bae9f1aad67c72879af588dce6bd9894e))
* build stacked branches on their parents, dependency-driven ([757a3d4](https://github.com/develar/branch-deck/commit/757a3d47aad5ef1a69d11f77f067c155d19da9ca))
* check the code-owner review before a Patronus Safe Push ([2614033](https://github.com/develar/branch-deck/commit/26140337d2cbde207244e3e541ac0af6975b3763))
* **cli:** a console bd.exe next to the app on Windows ([081a32f](https://github.com/develar/branch-deck/commit/081a32fd1c9ead9b50863b5b18263f2c29a95dfe))
* **cli:** bd check ([364ee14](https://github.com/develar/branch-deck/commit/364ee14d419597541c214466b02e5d7d84c5b792))
* **cli:** bd commits --brief and bd status --brief, commit filters, and bd version ([3321b4c](https://github.com/develar/branch-deck/commit/3321b4c928cca14c3828e4a557f9b45a385bb806))
* **cli:** bd group --sync and descriptions in the plan make grouping one call ([521c78f](https://github.com/develar/branch-deck/commit/521c78fb945130764b88e40a1cd9c046cd3430af))
* **cli:** bd group, rename, restack, unstack and undo ([b4c5b28](https://github.com/develar/branch-deck/commit/b4c5b28d5895b817c33a356bdf3dddc81ee51bff))
* **cli:** bd install and Help → Install Agent CLI… let agents find bd ([8a45377](https://github.com/develar/branch-deck/commit/8a45377ff1813e967ab87bcee764e629f3fd8448))
* **cli:** bd push --remote compares with that remote as with origin ([118ea79](https://github.com/develar/branch-deck/commit/118ea791c393219dde55a52f3185e305fe1698f0))
* **cli:** bd push --with-children pushes the branches stacked on the named ones ([1504631](https://github.com/develar/branch-deck/commit/15046318b041a98f46fc83b964dceeeaab0c0c6a))
* **cli:** bd push leases on the remote tip its sync saw, and --expect-remote ([c3eb23e](https://github.com/develar/branch-deck/commit/c3eb23e9856ef366bc55aacfe5b96547b04b5197))
* **cli:** bd push with a recorded lease per ref ([bc78fa5](https://github.com/develar/branch-deck/commit/bc78fa5d0abfaf5ee3ee1d19d880c04978dce82a))
* **cli:** bd suggest proposes a checked grouping of the commits into branches ([eb7df1e](https://github.com/develar/branch-deck/commit/eb7df1e20fcd2d43015d4c5ff566de582dc0f888))
* **cli:** bd update applies a clean update, bd undo reverts it, and bd fold folds fixups ([9f5dde5](https://github.com/develar/branch-deck/commit/9f5dde5c44149cad56ab00fa444416ef5f95109e))
* **cli:** bd update resolves a conflicting update in a resolve session outside the working tree ([0ec09f5](https://github.com/develar/branch-deck/commit/0ec09f5dcffaf88188e7baa5c9a550189be2110f))
* **cli:** leave lock files and generated files out of dependencies ([c099082](https://github.com/develar/branch-deck/commit/c099082044a18a7c2bbca651994c98816ac8457e))
* **cli:** stacks in status and commits, bd describe and bd sync ([16c038e](https://github.com/develar/branch-deck/commit/16c038ed780cfd0c1630c0561a56e7730a8780df))
* **cli:** status --stats per branch ([36d90e9](https://github.com/develar/branch-deck/commit/36d90e9311792fa7570cd8f1acf9b8e85dd746a6))
* **cli:** status places stacked branches as the stacked sync does ([4da5df7](https://github.com/develar/branch-deck/commit/4da5df7db55027974e75413862b9c5cf4fef855a))
* commit preview opened with Space instead of the inline body ([dd4ec04](https://github.com/develar/branch-deck/commit/dd4ec04df4fd9b571d1577303d47b4f9ff2d38e6))
* commit rows count the changed files and the commit preview lists them with diffs ([7128758](https://github.com/develar/branch-deck/commit/7128758aaa1d8e0bbc05f57a1e2088614706583f))
* **commit-files:** the files of a commit as a directory tree, their diffs from one patch of the commit with expandable context ([f7a0b4b](https://github.com/develar/branch-deck/commit/f7a0b4ba36998333ef9d453f40e5b315078c5b6e))
* **commit-review:** D reviews a commit: the directory tree beside the diffs of all its files, J/K files, N/P changes ([f863b0d](https://github.com/develar/branch-deck/commit/f863b0d9d57b1db1e05178313e0dc4968da848bb))
* detect an archived stacked branch from its stack base ([044c930](https://github.com/develar/branch-deck/commit/044c93053deda23a0a37114db007ad210862a842))
* **diff:** every diff on @pierre/diffs, with word diff, IntelliJ syntax colours and Shiki in a worker pool ([7dbcb94](https://github.com/develar/branch-deck/commit/7dbcb9420976f64b9f815e855de8d273756b5138))
* edit a commit message in the commit preview ([24987bc](https://github.com/develar/branch-deck/commit/24987bc3f071c538826f3e7fa4dfc56bdda3d075))
* **header:** robots indicator and Dock ([e2da8d2](https://github.com/develar/branch-deck/commit/e2da8d259c36c05953f9b824e6c134703a59cce4))
* inline commit message bodies with reading typography ([879f58b](https://github.com/develar/branch-deck/commit/879f58bdd895ea52f0e57be99d2c21ec1fe37f68))
* **install:** one command installs and upgrades the app, no Gatekeeper or SmartScreen step ([a4425f2](https://github.com/develar/branch-deck/commit/a4425f2d717ef1c2486532503e29fcd8ff98a742))
* integration status ([5ce7e7b](https://github.com/develar/branch-deck/commit/5ce7e7b9de4443b5fd2e9dc5aa82d84f40b8d1eb))
* issue groups put the commits of several issue keys into one branch without a rewrite ([4437823](https://github.com/develar/branch-deck/commit/4437823812934455caa3ff2a9a1dddc153319012))
* keep an Activity log of what Branch Deck did, and hide the Landed island once seen ([bce222d](https://github.com/develar/branch-deck/commit/bce222d615d9bf71a17f9a026c76e76c7c20e7a4))
* merge conflict viewer ([3ce1b34](https://github.com/develar/branch-deck/commit/3ce1b34dd537f338f6fb67d143e93dc43025e69e))
* merge conflict viewer (fix sub windows) ([fdc13b3](https://github.com/develar/branch-deck/commit/fdc13b3dc5954d310944dcf59a2b2bb69fc102c2))
* **model:** switch to Qwen3-4B-Instruct-2507 and remove obsolete model caches ([b6b55a7](https://github.com/develar/branch-deck/commit/b6b55a780c61e31c56de71a3e34715b15f7b4a9a))
* move commits and whole branches into one branch with the smallest change, from every gesture ([6421379](https://github.com/develar/branch-deck/commit/642137915a8edbedd587c202d743406f38a67a34))
* move commits between branches, split an issue branch into pushable branches ([45a889e](https://github.com/develar/branch-deck/commit/45a889edaab4309766096fbab8d4e0a90dd3b8e6))
* object cache keyed by git object ids, used by bd commits and status --stats ([e203842](https://github.com/develar/branch-deck/commit/e20384284dc78c0da8aedaa9c1aa191e46bb16d1))
* onboarding from the first window to the first sync ([24bfaa4](https://github.com/develar/branch-deck/commit/24bfaa425f876ebdaf9d69edad67fc1785f1aa2e))
* one commit row anatomy, quieter branch table and conflict viewer, wider body ([11c6a17](https://github.com/develar/branch-deck/commit/11c6a17e2f46db06e2f9ac8ed871d9f396362601))
* one push lease for the app and bd push ([752099c](https://github.com/develar/branch-deck/commit/752099c63746e6f66e21b2c893a3d3613221d7b1))
* **outgoing:** keep branch names readable and explain the attention dot ([f61bd84](https://github.com/develar/branch-deck/commit/f61bd84793780e113eab42ecf3730415bbb6eee1))
* Patronus robot status per branch ([0a8af46](https://github.com/develar/branch-deck/commit/0a8af46f2e7e26a601cca2e837a9fb23ede2009e))
* **patronus:** merge a GitHub pull request through its /patronus comment ([fda451a](https://github.com/develar/branch-deck/commit/fda451a49c0fb182ff18ef7b3a7974c19bee58be))
* pull request bases for stacked branches ([88cd38f](https://github.com/develar/branch-deck/commit/88cd38f692d7cfaee529735ec7ed6372a34f55e6))
* **pull-requests:** name the GitHub repository in an available answer ([eca3494](https://github.com/develar/branch-deck/commit/eca34943c22140673f98906492dec627329f4f8f))
* **pull-requests:** read the GitHub mirror from the committed Space settings ([996b74c](https://github.com/develar/branch-deck/commit/996b74c21ea07e719d3805064fadb61677ef39ce))
* push reports its progress, as the fetch does ([cbdc421](https://github.com/develar/branch-deck/commit/cbdc4211f677eaf9c9892b53a9f48bfbd4c0109b))
* **react:** auto-update, menu sync, auto-sync on focus and theme dialog ([7b28a3d](https://github.com/develar/branch-deck/commit/7b28a3deedd92d918f80246f51abf743a543d16c))
* **react:** branch sync store, branch tables and inline row forms ([9ed746f](https://github.com/develar/branch-deck/commit/9ed746f4bdc0131f6207f7714b878e98e178b2c1))
* **react:** commit list, row selection and inline branch creator ([7ffc7ba](https://github.com/develar/branch-deck/commit/7ffc7ba1597314f10b0c3cdc01d232131f3300c3))
* **react:** conflict viewer and AI branch naming ([1e52bbf](https://github.com/develar/branch-deck/commit/1e52bbf1f1de842cde9da0c1ac389715c11d7342))
* **react:** core infrastructure, settings stores and app shell ([8452005](https://github.com/develar/branch-deck/commit/84520058d4fa1879d203a925b78879d4eb564fc6))
* **react:** repository selection, configuration header and welcome card ([3f9bd6f](https://github.com/develar/branch-deck/commit/3f9bd6fc1ba11308afc68884243761fd53a8b837))
* read stacked branch prefixes in the grouper, opt-in ([37d6b7a](https://github.com/develar/branch-deck/commit/37d6b7a80e8ae4698b9f9800a6f1eda70b2f6075))
* readable commit message popover with reflow and scrolling ([50177f4](https://github.com/develar/branch-deck/commit/50177f40646aa719017dca293b5e30b852fecf00))
* regroup, rename and restack commits in one validated rewrite ([ed155dc](https://github.com/develar/branch-deck/commit/ed155dc2f742cdedaae16db44e5373052d52e9a1))
* remote commands that accept a non-zero exit code ([d5ce26e](https://github.com/develar/branch-deck/commit/d5ce26e23767bbea50f7b2039bedcfbe13e57771))
* rename and restack commands for the UI ([42fed6c](https://github.com/develar/branch-deck/commit/42fed6c2796f4e71fd3e07cd3520fb61a039eb9b))
* report each branch's place in its stack ([d2da147](https://github.com/develar/branch-deck/commit/d2da147e513d807f9e2187761ab7abd588d1a028))
* retarget a pull request whose base is not the branch's parent ([6bc7c62](https://github.com/develar/branch-deck/commit/6bc7c62cd03bce38177dd08dc7dba4906c4ba0da))
* rewrite history without touching the working tree, require Git 2.54 ([c6c6c79](https://github.com/develar/branch-deck/commit/c6c6c79d3c2890e3f414d9f26368453153f580b5))
* **row:** compact actions and the live Safe Push slot ([7142176](https://github.com/develar/branch-deck/commit/71421760ed841a311632f393397e299b7c75e224))
* show GitHub pull request status per branch ([6357cc9](https://github.com/develar/branch-deck/commit/6357cc9ac6e45923b5dea98552748060554ecbcd))
* show outgoing commits in the header ([d402090](https://github.com/develar/branch-deck/commit/d402090a813c713d0b47fbe4641abc6c6d5bc207))
* Space preview on branch rows ([a460908](https://github.com/develar/branch-deck/commit/a460908183c5ece9cba0f89cc2f885f0125f471c))
* squash fixup commits ([ee7d884](https://github.com/develar/branch-deck/commit/ee7d88404c79ad854c914f6e377b2e2d19246b41))
* squash selected commits ([28038e6](https://github.com/develar/branch-deck/commit/28038e6018146fa7ef915d6a8e0bd794c07a0043))
* stack-aware Move to… and commit message hint ([4aa5bd8](https://github.com/develar/branch-deck/commit/4aa5bd8ac832cc57786873094fd0a57682275e6c))
* stack, unstack and rename branches from the branch menu ([feee9b2](https://github.com/develar/branch-deck/commit/feee9b2983eac67a834235409ed345f5142c6f4d))
* stacked branches as a tree in the branch table, blocked ones muted ([52c21e6](https://github.com/develar/branch-deck/commit/52c21e6f808030d32b0a728aa62389769158050d))
* **stack:** Push with Children pushes a parent with the branches stacked on it ([e464cf8](https://github.com/develar/branch-deck/commit/e464cf8c3ddc73c522fcc7d443d05fbe2b22f2ba))
* **stack:** the push button pushes the stack, Safe Push leads root first, pull requests follow the stack ([09dbfd2](https://github.com/develar/branch-deck/commit/09dbfd22bd90f706e957f3e9bcec62fed4a9eeaf))
* start, restart and cancel Patronus robots; ask before a push breaks a Safe Push ([15a10bf](https://github.com/develar/branch-deck/commit/15a10bf8e1b0be87c133227c46e09290362eec3f))
* support fixup! commits ([9b64d85](https://github.com/develar/branch-deck/commit/9b64d85885ff89491aee8d7361ba7232e1963868)), closes [#51](https://github.com/develar/branch-deck/issues/51)
* **sync:** a branch's description title is its summary in the table and the preview ([640d26a](https://github.com/develar/branch-deck/commit/640d26a7af80d97f8f1faaa8bab2ccd23824207c))
* **sync:** sync a repository synced before when the window opens it ([f937e53](https://github.com/develar/branch-deck/commit/f937e532491e3df24eaa4be63af92369c1924fbd))
* **table:** filter the branches by text and state, and walk the rows with the keyboard ([c1e9b7c](https://github.com/develar/branch-deck/commit/c1e9b7c3e7f06450766fd2b02dc0002fb007fc21))
* **table:** the stack graph replaces the staircase ([07f877e](https://github.com/develar/branch-deck/commit/07f877ea3720c7f416279b7c8a315f8ec46ccc2c))
* tell what of your work landed in the baseline, before and after the update ([a5cfa16](https://github.com/develar/branch-deck/commit/a5cfa16b38bb0cdd4c868698553e29bc27fe6305))
* tell when a Safe Push merged, also from the background ([92a6fab](https://github.com/develar/branch-deck/commit/92a6fabc517af003cba5e26e11588c76ccfcb441))
* the archived branch names the commit that merged it ([63b1179](https://github.com/develar/branch-deck/commit/63b1179d3eeb89dc6adc959725773a5fea351fed))
* the copied prompt for a blocked update sends the agent to bd ([1c55df1](https://github.com/develar/branch-deck/commit/1c55df17d940338978411714c9a4c67672b72df3))
* the Force Push description lists the remote commits it drops ([daa13ae](https://github.com/develar/branch-deck/commit/daa13ae8dcb9f983c9c62c231fb055707c1ef044))
* the remote status lists the commits a push drops ([bb196d7](https://github.com/develar/branch-deck/commit/bb196d77bdc0dfc93695d4fec29fe68f51ce03c6))
* the update conflict dialog shows who changed each conflicting file ([f1a169e](https://github.com/develar/branch-deck/commit/f1a169ef3204aaa21c48cdb3c32aa4793a879414))
* trial sync of a regroup plan, with merges memoized by their trees ([32d148d](https://github.com/develar/branch-deck/commit/32d148d01d0c9608a38b3689bd5a6cd230c002d6))
* **ui:** give toasts one vocabulary and open the object they are about ([2fdf172](https://github.com/develar/branch-deck/commit/2fdf172210669c4fc2e1f2252a1f436121c120f6))
* **ui:** inline forms with a visible title, progress, Enter to confirm and the focus back; form commands end with "…" ([d92471e](https://github.com/develar/branch-deck/commit/d92471e66d9843cdccfbc4d4475de99c9156510d))
* **ui:** push button keeps its colour, describes itself when disabled, asks before dropping remote commits ([4302584](https://github.com/develar/branch-deck/commit/4302584faa0225e3f96dc9b03f4a429987b210c4))
* **ui:** Safe Push and "⋯" in the row, Create Merge Request and Create Pull Request ([5ab3ae8](https://github.com/develar/branch-deck/commit/5ab3ae84e9068ab938cb5c6bcead007f8bbab2c7))
* **ui:** shared Callout, InlineCode, HelpPopover and dialog frame widths ([7ed5c4a](https://github.com/develar/branch-deck/commit/7ed5c4ac735802ef6dd01ddcc225f3db04afde08))
* **ui:** the toolbar is the title bar of the main window ([6312d90](https://github.com/develar/branch-deck/commit/6312d90c9a24122726263bbdbae8dba8cf9b7dbd))
* **ui:** Zoom In, Zoom Out and Actual Size in the View menu ([4755b31](https://github.com/develar/branch-deck/commit/4755b315dd1b78c6db12e6687f6277dee766a4e7))
* unassigned commits ([52a9c7d](https://github.com/develar/branch-deck/commit/52a9c7da384cc30a5befa7820581024eb362daa0))
* unassigned commits - AI branch name generation ([a6324ff](https://github.com/develar/branch-deck/commit/a6324ff9db665e285781b0a83b196de618ab5c06))
* update project onto the upstream without touching unrelated local changes ([ff366ae](https://github.com/develar/branch-deck/commit/ff366ae50af6385c12664dbc118dab40868c07d4))
* Update Project shows the fetch progress, the way IDEA does ([3a0431f](https://github.com/develar/branch-deck/commit/3a0431fc4173c4a24e77d6c256ea9e17777ea4e0))
* **update:** show when an upstream commit was authored and when it landed ([6cda46d](https://github.com/develar/branch-deck/commit/6cda46df214e1d54451ce4af30a437bc3a609719))


### Bug Fixes

* "1 of your commits" in the remote status description ([09d3692](https://github.com/develar/branch-deck/commit/09d36928f6bdc034c26e519e519d3ea15380c91d))
* a plan that keeps an issue commit in its own issue branch rewrites nothing ([91d4d07](https://github.com/develar/branch-deck/commit/91d4d07cdae134a25df4fa8f59f25ae65e802a63))
* a Safe Push started from the form cancels the running one instead of restarting it ([597d7b8](https://github.com/develar/branch-deck/commit/597d7b8d42653c9bc022c354971bdc62a4ff064e))
* a Safe Push that folded fixups into their target no longer makes Update Project conflict ([5e28ac9](https://github.com/develar/branch-deck/commit/5e28ac901caf6b6690d7faad75a5e31fb65f266d))
* a write of the issue groups keeps the keys of other branches, and an undo token is read strictly ([d169ad6](https://github.com/develar/branch-deck/commit/d169ad6f7c3d29900e21f79a20661775bae4eaa3))
* **a11y:** buttons for hover triggers, named repeated labels, one focus ring, data-status ([3ba4895](https://github.com/develar/branch-deck/commit/3ba48950cfdba62ae247dc7a8fe81dc2d718083d))
* AI branch name suggestions name one branch, without a slash ([d5147d2](https://github.com/develar/branch-deck/commit/d5147d2be091b7f503eb12bec1cf975e2ee88813))
* **ai:** plain texts, one chosen-chip style and spinner placeholders in the AI controls ([26465e7](https://github.com/develar/branch-deck/commit/26465e7c8ae592b266dcd9c72061ab9a47b0a60e))
* **amend:** restore the working tree after an aborted autosquash rebase ([18a66b4](https://github.com/develar/branch-deck/commit/18a66b424816054d0585fcb1afc337add3086b97))
* archive and unapply names avoid ref directory/file conflicts ([d41724a](https://github.com/develar/branch-deck/commit/d41724a3a08be16239c9f6daed87cd7c2fa1805b))
* **build:** export_bindings only in debug and test builds, a release build denies the dead code ([8624a6d](https://github.com/develar/branch-deck/commit/8624a6d92511434dbe1b4427c04302308b949909))
* case-insensitive git config ([31e39d4](https://github.com/develar/branch-deck/commit/31e39d430af171be3c02f63502f11093ebaf12c3))
* **claude:** the login shell gets 5 s to print its PATH ([326aaa0](https://github.com/develar/branch-deck/commit/326aaa09116653114379f2f77d93fb56a0eaeee4))
* **cli:** a failure after a bd group rewrite is the error of its output, and descriptions keep what they replace ([56970c3](https://github.com/develar/branch-deck/commit/56970c34ffd2e268f75d14c2125c5e384f8b9d50))
* **cli:** a resolve session leaves out commits the upstream has by patch id, resolves rename/delete, and names the commit of a refusal ([50d25d1](https://github.com/develar/branch-deck/commit/50d25d1e11c086219edd7fd187f72c25575e6b78))
* **cli:** bd suggest hashes its dump with a random SipHash key and repairs conflicts inside issue groups ([8bcb05f](https://github.com/develar/branch-deck/commit/8bcb05f0b5da24d7c529bbaa7bf6ea9ec04922f7))
* **cli:** bd suggest stacks a branch on the branches it needs, so its plan passes its own check ([93c14ce](https://github.com/develar/branch-deck/commit/93c14ce9490faf3a20ec2928328836a257e6e157))
* **cli:** resolve rounds read only bd's own markers, never drop a commit silently, and never strand a moved session ([0d16591](https://github.com/develar/branch-deck/commit/0d165914739816f8c4838ae50aab6fbed0d5a283))
* **cli:** the brief commits view keeps a stack prefix, and the schema requires no field an output leaves out ([18c5cc4](https://github.com/develar/branch-deck/commit/18c5cc41d40d0ea9aec3daab3cc32cf511e70a53))
* **cli:** the next update finishes an update killed before its files followed, and bd reads and writes text that Windows PowerShell 5.1 handles ([a462e77](https://github.com/develar/branch-deck/commit/a462e7748d8352d0d7638e1c23e4ae9378026814))
* **cli:** the update moves HEAD only under the HEAD lock, the undo is sealed, and a blocked prompt never stashes ([45b36a7](https://github.com/develar/branch-deck/commit/45b36a7789b0aebb1a10144bdede0573b0735e06))
* compare a robot and a pull request with the remote branch, not with the local hash ([9af2329](https://github.com/develar/branch-deck/commit/9af232953ba02156f867b6d5326836b932538f85))
* **conflict:** nested panels, one conflict title and insight, and no empty Diff View ([f86a516](https://github.com/develar/branch-deck/commit/f86a51636ea2076824e730269ef874e0acd07e62))
* **conflict:** pad the commit card of a sub-window inside its body ([197443a](https://github.com/develar/branch-deck/commit/197443ae6f62fb4803c4e6b0ab19f6890a6d0c42))
* **conflicts:** a root commit diffs against the empty tree of the repository's hash ([ad29eb5](https://github.com/develar/branch-deck/commit/ad29eb58d2317ef6b1fe41ee6d5ddaa51fc6c418))
* dedupe tokenizers and apply toast theme override ([135807c](https://github.com/develar/branch-deck/commit/135807c172626b0ed46072fae6df82a18305b4b4))
* **detection:** a NotIntegrated note always lists its orphaned commits, an archived stack parent is checked again before the placement, and a fast-forwarded branch names its tip ([56c6b98](https://github.com/develar/branch-deck/commit/56c6b9854585e0d66a5c7886c4b504dc9975bab1))
* disable add issue reference action if all commits have issue number ([d6c4748](https://github.com/develar/branch-deck/commit/d6c47489c704795f0c595f8f8a3a8f7508416edc))
* don't claim an update state before the first fetch, nor offer "fetch and update" while one runs ([4b6b8de](https://github.com/develar/branch-deck/commit/4b6b8ded15b67ebd7eafe2b41332f714c197919c))
* **e2e:** a commit list is aria-busy while its file counts load, and HTML snapshots wait for it ([6f27b06](https://github.com/develar/branch-deck/commit/6f27b062c6566b26c805aa921c2463b3d599802f))
* **e2e:** a green run logs nothing: the cleanup waits for running commands, and a failed command is its answer ([029052f](https://github.com/develar/branch-deck/commit/029052fb40013af9b0b391f1e47bd6df2fdead56))
* **e2e:** a never-fetched baseline says it was not fetched yet ([b2389e8](https://github.com/develar/branch-deck/commit/b2389e8e99b2723ac3812882cdf760e01f0cc6ae))
* **e2e:** a quiet mock server, its own port per checkout, and no race with the 200 ms copied state ([4fde72e](https://github.com/develar/branch-deck/commit/4fde72ef23e0011d4a067798b20579b33b50775e))
* **e2e:** an HTML snapshot waits for closing popups, not for collapsed accordions ([d8467a9](https://github.com/develar/branch-deck/commit/d8467a912eb5f3dcf76870bc23612936b84a7b14))
* **e2e:** close the push description after a push from the row ([87e6321](https://github.com/develar/branch-deck/commit/87e6321884960cdfafc122ca44875de53bd64d60))
* **e2e:** confirm the force push that drops a remote commit in the Activity test ([e16616c](https://github.com/develar/branch-deck/commit/e16616cd0a328bca44ecc5d911aba6517bfb01f3))
* **e2e:** hover the common ancestor hash once the unassigned commits are above the table ([634a44d](https://github.com/develar/branch-deck/commit/634a44da3690f66e120bbbfc7ca38dc1af681239))
* **e2e:** keep the times of the update templates before the page clock ([56e875b](https://github.com/develar/branch-deck/commit/56e875bdff0ec2e6c17561d35c98bb63a39060c3))
* **e2e:** move the pointer off the push button after a push from the row ([abe9ac5](https://github.com/develar/branch-deck/commit/abe9ac5290b3ce09faf8e190e1665ced63e077cd))
* **e2e:** press the squash shortcut once the message editor has the focus ([b329c45](https://github.com/develar/branch-deck/commit/b329c4544eea826a166171a1de6201d80fc971dc))
* **e2e:** read the open fixup tooltip, not the closing one of the expand button ([03f74e3](https://github.com/develar/branch-deck/commit/03f74e3af64c8e607323ed2120f9094e4215e0d2))
* **e2e:** read the title of the commit preview as its level-2 heading ([4512738](https://github.com/develar/branch-deck/commit/4512738ebba96a3f6eb3b4551b50b83c247421bd))
* **e2e:** the merged-by line of an archived branch has no time ([f4e47ee](https://github.com/develar/branch-deck/commit/f4e47eef6196a845d69bd60be98cb74e7691c4c9))
* **e2e:** the Restart of the robot details is named "Restart Safe Push" ([8038044](https://github.com/develar/branch-deck/commit/803804415c22d677652da03fdfb8cd5507dd997b))
* **e2e:** the simple template's test-branch has one commit ([d6dd95f](https://github.com/develar/branch-deck/commit/d6dd95f00220baf35fb28e09d09b4e870aac77d8))
* **e2e:** wait for the merged status word before expanding an archived branch ([769f793](https://github.com/develar/branch-deck/commit/769f7937f1370c1aeeb8d3fd905ef122b0e89ae1))
* **e2e:** wait for the robots and for the push guard's focus in the Patronus actions ([a6b1fd5](https://github.com/develar/branch-deck/commit/a6b1fd53183152b6b20dd6cb670a263bc8a2efac))
* **git:** a dropped interactive git exits before it is killed, so it leaves no ref locks ([02a78b5](https://github.com/develar/branch-deck/commit/02a78b5ef36167327a519e3b8209bab1aa63f531))
* **git:** an update refuses a HEAD switched to another branch since the check ([59a34b4](https://github.com/develar/branch-deck/commit/59a34b44984f8a5fb1e87118c2de4d4bd56b1505))
* **git:** mapping notes are read from their own notes ref only ([ef230b8](https://github.com/develar/branch-deck/commit/ef230b805112b2c58ffdd52cbb8eb41ccdfb3339))
* **git:** resolve rounds read the markers of a renamed file and report a lost one-word change ([c4a3f65](https://github.com/develar/branch-deck/commit/c4a3f65191722b5c3d8192de2040df83a8836469))
* give each landed number its verb, in one grammar on every surface ([c7d7a92](https://github.com/develar/branch-deck/commit/c7d7a92d10fba453bb7a6827e877619adac095dd))
* give the commit title the free width of the branch table ([5c55331](https://github.com/develar/branch-deck/commit/5c55331e426deb82b8cbcd325d1a0a67f42bfd7d)), closes [#72](https://github.com/develar/branch-deck/issues/72)
* **header:** keep the header on one line from 960 px and still during a sync ([5f18c89](https://github.com/develar/branch-deck/commit/5f18c89278854a8d4ae4100389fb6b910df3831c))
* history rewrites move the checked out branch only if it did not move ([698ceb5](https://github.com/develar/branch-deck/commit/698ceb599c3e18dd851cca4282392c267dca88a3))
* inline action progress ([6f89681](https://github.com/develar/branch-deck/commit/6f89681c1c00bc90b4ac014686c65a98bd493c49))
* keep archived branches newest first ([9b9bc33](https://github.com/develar/branch-deck/commit/9b9bc33fd86b48d1afeaf286950e0195da307814))
* keep the attention for a failed Safe Push after a sync rebuilt the branch ([4b552ee](https://github.com/develar/branch-deck/commit/4b552ee2223d1526b38e5b56ec50a80d54d61828))
* keep the push state of a branch across a sync ([7fafdfb](https://github.com/develar/branch-deck/commit/7fafdfbf9a971c1f2d080782d0b8c87204529eeb)), closes [#71](https://github.com/develar/branch-deck/issues/71)
* keep the reword error the UI shows for a commit that left the branch ([dcd299b](https://github.com/develar/branch-deck/commit/dcd299b52a543f71dacd42eaeb7b88f9a6617721))
* make AI feature more visible ([ec22728](https://github.com/develar/branch-deck/commit/ec22728dff6bfa28ecf9bc17d58ded8c46278011))
* **model:** keep the partial download when the user pauses ([61a38f5](https://github.com/develar/branch-deck/commit/61a38f5d18895976fce674e24b038957a9dfbb01))
* move auto-update logic from js to rust to improve security ([b937316](https://github.com/develar/branch-deck/commit/b937316ee19729f90bbe8209de63d6bba40f73fe))
* one colour rule for push states ([4e3455c](https://github.com/develar/branch-deck/commit/4e3455c801d8deb8cb319e17a8722348d30d049b))
* outgoing counts the work the remote lacks, not commit hashes ([54d9edd](https://github.com/develar/branch-deck/commit/54d9edd2e812e709aca14f8f383420f9ad08afc7))
* **outgoing:** the review chips of the overview never overlap the state word ([37ead8a](https://github.com/develar/branch-deck/commit/37ead8a0a25b5c49abf4ebf54c53eba352a54947))
* **patronus:** a robot id from the page is one escaped path segment, a comment id a number ([06ee747](https://github.com/develar/branch-deck/commit/06ee747e7683da86af52019bd8c2aa50959fbb0a))
* **patronus:** a start right after a push is not asked again ([3930b8b](https://github.com/develar/branch-deck/commit/3930b8b12e83f8b3066e1c39dd0b9f4789ef8c87))
* **patronus:** say what a dry run leaves out and which commit the start form pushes first ([8658933](https://github.com/develar/branch-deck/commit/8658933626e48b56f460ca31c28da053b4053798))
* **patronus:** the cached answers are bounded moka caches ([61e0116](https://github.com/develar/branch-deck/commit/61e0116ae83b0201aa6d87ec3adf77e286514972))
* **patronus:** the review button names Space, and the block says why ([d9343dc](https://github.com/develar/branch-deck/commit/d9343dc454c9f45b6915a961d65c40b88587147f))
* push replaces the remote branch only as the row showed it ([992485e](https://github.com/develar/branch-deck/commit/992485ecd1798cacc5f461404372821e63b2f990))
* **push:** a Force Push of an amended or restacked branch names what it replaces once ([26f9d93](https://github.com/develar/branch-deck/commit/26f9d936271bac6aa121a5d11cc26680e51ae23f))
* **push:** a stack push names what each force push replaces or drops ([886081a](https://github.com/develar/branch-deck/commit/886081a3b79badb27e7b026008a18aa8f5bef5f1))
* **push:** count a push once and say what it does ([e06fa79](https://github.com/develar/branch-deck/commit/e06fa79b3a82ee517a07085111483f63057c76a0))
* **push:** keep the user's credential helpers, never prompt ([2b689a8](https://github.com/develar/branch-deck/commit/2b689a821841fb753b6c44bed280fb1d4402ab1a))
* **push:** one toast per action, and Create Pull Request after every push ([ed704de](https://github.com/develar/branch-deck/commit/ed704de18dc9c2843a02f6e7529d18dbf8e26e70))
* relative times age while they are shown ([0be537d](https://github.com/develar/branch-deck/commit/0be537db1227c92b765818eae54da370994c4ba9))
* relaunch after auto-update ([da5ec34](https://github.com/develar/branch-deck/commit/da5ec34bcde63b7a8eb774ad2dfd9d73e6a139da))
* remote status - use path-id based strategy ([8b3292a](https://github.com/develar/branch-deck/commit/8b3292acd0de85d4fac7bd67329ea76d1fc9e04c))
* **remote-status:** an amended or restacked commit is replaced, not dropped, and no longer warns ([250a133](https://github.com/develar/branch-deck/commit/250a1335959a6235f88c9b0925ea01a8dbadbd53))
* **resolve:** a tree path that NTFS or HFS+ reads as .git is invalid ([a0b5760](https://github.com/develar/branch-deck/commit/a0b57605f0501bcd277a5181ca019e6a8e93642b))
* reuse a virtual commit only on its parent and write deterministic copies ([89960d7](https://github.com/develar/branch-deck/commit/89960d7f79133bd7ba807333d29626f99f357d17))
* show the copy buttons of the baseline and merged-by commits on hover ([c06c84e](https://github.com/develar/branch-deck/commit/c06c84ebf0a63c709a65583b6ee2f5a42aabfaa5))
* show the issue links of commit subjects in compact lists ([8f0a0be](https://github.com/develar/branch-deck/commit/8f0a0bef2baabbea3b6d7a1ce84c7d0ab50b453d))
* slow cherry-pick ([252f076](https://github.com/develar/branch-deck/commit/252f07683fd0bd7ed29b520a7324f7de1e8992f9))
* **stack:** a parent without commits never blocks its children, and a NotIntegrated note is checked again when the baseline moves ([dccf7ac](https://github.com/develar/branch-deck/commit/dccf7aceb33dd8398ae90ab187bb34d20557ea2f))
* **stacks:** the first sync after a parent's merge moves its child ([543c2b2](https://github.com/develar/branch-deck/commit/543c2b2d523432a0387816e8ac61977db9b3f3b3))
* **suggest:** an issue key is cut from the name words by position ([b04fce3](https://github.com/develar/branch-deck/commit/b04fce3d73639cb59de8fe6553f8038be02968c7))
* sync, archive and model bugs found in Rust review ([b17a407](https://github.com/develar/branch-deck/commit/b17a407cb6687d5b42d4fee0fb6016114f3f8211))
* **sync:** a stacked child's remote status excludes its parent by the head it was built on, not by the parent's ref ([8082501](https://github.com/develar/branch-deck/commit/8082501779f06e26b265d187546a93db29c25caa))
* test server serves only the dist of its own workspace ([1ea6c1f](https://github.com/develar/branch-deck/commit/1ea6c1f51a5c6cd8cb67cf492aa5da7282db729b))
* **test:** fixture git ignores an inherited GIT_DIR, so a test run under rebase --exec can't write into the clone ([1f03970](https://github.com/develar/branch-deck/commit/1f0397031af872eeca42c6b6f94678d8c8f32978))
* the conflict prompt names bd once and covers the blocked paths, the pending finish and exit 2 ([fd599cb](https://github.com/develar/branch-deck/commit/fd599cbfb7a7254f40414ade908f5667a102a132))
* the dropped-line report looks where a block was, the prompt commits a fixup by its paths, and the hints fit the field cases ([62b99a2](https://github.com/develar/branch-deck/commit/62b99a2a96824f2e7a12ece941261b68d859b3a7))
* the outgoing overview footer names the update state as incoming, with the fetch time ([df4aeda](https://github.com/develar/branch-deck/commit/df4aedadb2ec56befeb43e245e05273fe51db166))
* the Update button tooltip names the running step, not the last state ([9fcd46b](https://github.com/develar/branch-deck/commit/9fcd46b87146e27b8923ba9f14b9d6bc97f171b7))
* the update step and fetch progress belong to their repository ([10a84f8](https://github.com/develar/branch-deck/commit/10a84f8b28347af494f3bc3a31a77d7bdc95462d))
* truncate commit message text ([a9bcecf](https://github.com/develar/branch-deck/commit/a9bcecf4c9e789e00a9d96e532cd5d5ae6e8c37e))
* **ui:** align the branch row status cell, the chip anatomy and the tree spine ([9cb4f36](https://github.com/develar/branch-deck/commit/9cb4f36e4ea46ab543d481095bea6e4ef3f7f308))
* **ui:** centre the traffic lights in the toolbar ([10d6ee5](https://github.com/develar/branch-deck/commit/10d6ee5e8070e1445bca2a1e4fe987bd26c0e4ad))
* **ui:** commit bodies in the reading size, code at the size of its text ([957f474](https://github.com/develar/branch-deck/commit/957f474ff2e3fa74aa56d6596d15b1d9c0c5c9d3))
* **ui:** commit subjects in regular weight, so a commit list does not outweigh the branch names ([11df5c9](https://github.com/develar/branch-deck/commit/11df5c9566659ef1818cf58fcc04db0032b292de))
* **ui:** one icon per concept and tooltips on icon-only buttons ([543c89c](https://github.com/develar/branch-deck/commit/543c89c49c690ca62655047d733468f517ab6661))
* **ui:** one meaning per colour ([eb18100](https://github.com/develar/branch-deck/commit/eb18100d300ccb5349f9a504e15103035b53de4e))
* **ui:** text in tooltips, rich content in popovers of one width ([1e77fdb](https://github.com/develar/branch-deck/commit/1e77fdb3d2f3b5cf0e40f09a8513f59d96992906))
* **ui:** the Activity button has a tooltip like every icon-only button ([3b70c07](https://github.com/develar/branch-deck/commit/3b70c073b0ba2ad348845a3eca9b997a499d1c67))
* **ui:** the archived commits load error is a text-sm line like the other states ([dd51ebf](https://github.com/develar/branch-deck/commit/dd51ebf502b577dea8552c15ae185f5c26b2088d))
* **ui:** the branch menu names the Space merge request as the review block does ([56e4711](https://github.com/develar/branch-deck/commit/56e4711477d1dc4855a2202aaff15e517d79b13b))
* **ui:** the path error says its message once, and the welcome steps count from 1 ([aa7ef05](https://github.com/develar/branch-deck/commit/aa7ef054f9723a6b04503c687295031f181cca6b))
* **ui:** toast text wraps long paths instead of running under the action ([cd89345](https://github.com/develar/branch-deck/commit/cd89345c966cd1c6cdce1a91dfead08e830c9f91))
* **ui:** type sizes from the scale, tabular live counters, mono hashes ([1229bcb](https://github.com/develar/branch-deck/commit/1229bcbfe97ab9dcd367d053f935c419b48d176e))
* **ui:** Zoom In shows ⌘+ on macOS, as in Safari and Slack ([a38c443](https://github.com/develar/branch-deck/commit/a38c443f6737c5c1f71e1ffdc14091797e354308))
* update remote sync on push ([dd7ad83](https://github.com/develar/branch-deck/commit/dd7ad8352b723744ab6da55435bd7337830a10ef))
* use `()` instead of `[]` as prefix because `[]` is a common convention for subsystems ([8a78518](https://github.com/develar/branch-deck/commit/8a78518cb4a045b882d8459c1b09729c09b6c004))
* use a correct base commit to avoid cherry-picking unrelated changes ([372ebe7](https://github.com/develar/branch-deck/commit/372ebe7bc4a6e4f03d9da0bd18a3d7efa3ede640))
* use author time ([bbe3aaf](https://github.com/develar/branch-deck/commit/bbe3aaf48bf9b2d9bd3ccac900818733413bab14))
* virtual commits keep the original timezones ([bebbd57](https://github.com/develar/branch-deck/commit/bebbd57fa5ec0232df2fb15685db945507a84274))


### Performance Improvements

* bundle common highlight.js grammars instead of all ([0137984](https://github.com/develar/branch-deck/commit/01379842b38c20096d20faa512363be7d3fb571e))
* carry mapping notes across rewords and bound the reword walk ([7beff05](https://github.com/develar/branch-deck/commit/7beff0500388a3ac02379a5f4ab67bb314d0ce1e))
* cheap update check during sync ([12ed3e8](https://github.com/develar/branch-deck/commit/12ed3e87fd482daf5399471d68a49818745d859f))
* e2e in about a minute instead of five ([203b23e](https://github.com/develar/branch-deck/commit/203b23e6cb36130865ff6ee557f168695733433f))
* merge using tree, no need to use commits ([4ba8048](https://github.com/develar/branch-deck/commit/4ba8048cca2d1797380f50ece1885b6f14600a4e))
* one ref listing and one notes write per sync ([92e7594](https://github.com/develar/branch-deck/commit/92e7594e712c9eb6d6933df9438172332a1d3500))

## [0.12.0](https://github.com/develar/branch-deck/compare/v0.11.0...v0.12.0) (2026-10-15)


### Features

* a stacked branch's unpushed commits exclude its parent's ([c34e0f4](https://github.com/develar/branch-deck/commit/c34e0f4c8aba3efa45f2c1c89562cab1f1ce594b))
* autosquash matches fixups by stack leaf ([15fefea](https://github.com/develar/branch-deck/commit/15fefeae2d666eaa3c40e825d631643cb9928a58))
* bd agent CLI with read-only status, commits, schema and help agent ([ce833ce](https://github.com/develar/branch-deck/commit/ce833ce06d2924cdba4324cbeeddd76bce6e76a9))
* blocked branches offer no push anywhere, stacked ones name their base ([662b3bd](https://github.com/develar/branch-deck/commit/662b3bd7796e7f9f7025de2ce47a5a1e85f63541))
* build stacked branches on their parents, dependency-driven ([757a3d4](https://github.com/develar/branch-deck/commit/757a3d47aad5ef1a69d11f77f067c155d19da9ca))
* check the code-owner review before a Patronus Safe Push ([2614033](https://github.com/develar/branch-deck/commit/26140337d2cbde207244e3e541ac0af6975b3763))
* **cli:** a console bd.exe next to the app on Windows ([081a32f](https://github.com/develar/branch-deck/commit/081a32fd1c9ead9b50863b5b18263f2c29a95dfe))
* **cli:** bd check ([364ee14](https://github.com/develar/branch-deck/commit/364ee14d419597541c214466b02e5d7d84c5b792))
* **cli:** bd commits --brief and bd status --brief, commit filters, and bd version ([3321b4c](https://github.com/develar/branch-deck/commit/3321b4c928cca14c3828e4a557f9b45a385bb806))
* **cli:** bd group --sync and descriptions in the plan make grouping one call ([521c78f](https://github.com/develar/branch-deck/commit/521c78fb945130764b88e40a1cd9c046cd3430af))
* **cli:** bd group, rename, restack, unstack and undo ([b4c5b28](https://github.com/develar/branch-deck/commit/b4c5b28d5895b817c33a356bdf3dddc81ee51bff))
* **cli:** bd install and Help → Install Agent CLI… let agents find bd ([8a45377](https://github.com/develar/branch-deck/commit/8a45377ff1813e967ab87bcee764e629f3fd8448))
* **cli:** bd push --remote compares with that remote as with origin ([118ea79](https://github.com/develar/branch-deck/commit/118ea791c393219dde55a52f3185e305fe1698f0))
* **cli:** bd push leases on the remote tip its sync saw, and --expect-remote ([c3eb23e](https://github.com/develar/branch-deck/commit/c3eb23e9856ef366bc55aacfe5b96547b04b5197))
* **cli:** bd push with a recorded lease per ref ([bc78fa5](https://github.com/develar/branch-deck/commit/bc78fa5d0abfaf5ee3ee1d19d880c04978dce82a))
* **cli:** bd suggest proposes a checked grouping of the commits into branches ([eb7df1e](https://github.com/develar/branch-deck/commit/eb7df1e20fcd2d43015d4c5ff566de582dc0f888))
* **cli:** bd update applies a clean update, bd undo reverts it, and bd fold folds fixups ([9f5dde5](https://github.com/develar/branch-deck/commit/9f5dde5c44149cad56ab00fa444416ef5f95109e))
* **cli:** bd update resolves a conflicting update in a resolve session outside the working tree ([0ec09f5](https://github.com/develar/branch-deck/commit/0ec09f5dcffaf88188e7baa5c9a550189be2110f))
* **cli:** leave lock files and generated files out of dependencies ([c099082](https://github.com/develar/branch-deck/commit/c099082044a18a7c2bbca651994c98816ac8457e))
* **cli:** stacks in status and commits, bd describe and bd sync ([16c038e](https://github.com/develar/branch-deck/commit/16c038ed780cfd0c1630c0561a56e7730a8780df))
* **cli:** status --stats per branch ([36d90e9](https://github.com/develar/branch-deck/commit/36d90e9311792fa7570cd8f1acf9b8e85dd746a6))
* **cli:** status places stacked branches as the stacked sync does ([4da5df7](https://github.com/develar/branch-deck/commit/4da5df7db55027974e75413862b9c5cf4fef855a))
* commit rows count the changed files and the commit preview lists them with diffs ([7128758](https://github.com/develar/branch-deck/commit/7128758aaa1d8e0bbc05f57a1e2088614706583f))
* detect an archived stacked branch from its stack base ([044c930](https://github.com/develar/branch-deck/commit/044c93053deda23a0a37114db007ad210862a842))
* issue groups put the commits of several issue keys into one branch without a rewrite ([4437823](https://github.com/develar/branch-deck/commit/4437823812934455caa3ff2a9a1dddc153319012))
* keep an Activity log of what Branch Deck did, and hide the Landed island once seen ([bce222d](https://github.com/develar/branch-deck/commit/bce222d615d9bf71a17f9a026c76e76c7c20e7a4))
* object cache keyed by git object ids, used by bd commits and status --stats ([e203842](https://github.com/develar/branch-deck/commit/e20384284dc78c0da8aedaa9c1aa191e46bb16d1))
* onboarding from the first window to the first sync ([24bfaa4](https://github.com/develar/branch-deck/commit/24bfaa425f876ebdaf9d69edad67fc1785f1aa2e))
* one push lease for the app and bd push ([752099c](https://github.com/develar/branch-deck/commit/752099c63746e6f66e21b2c893a3d3613221d7b1))
* **outgoing:** keep branch names readable and explain the attention dot ([f61bd84](https://github.com/develar/branch-deck/commit/f61bd84793780e113eab42ecf3730415bbb6eee1))
* Patronus robot status per branch ([0a8af46](https://github.com/develar/branch-deck/commit/0a8af46f2e7e26a601cca2e837a9fb23ede2009e))
* **patronus:** merge a GitHub pull request through its /patronus comment ([fda451a](https://github.com/develar/branch-deck/commit/fda451a49c0fb182ff18ef7b3a7974c19bee58be))
* pull request bases for stacked branches ([88cd38f](https://github.com/develar/branch-deck/commit/88cd38f692d7cfaee529735ec7ed6372a34f55e6))
* **pull-requests:** name the GitHub repository in an available answer ([eca3494](https://github.com/develar/branch-deck/commit/eca34943c22140673f98906492dec627329f4f8f))
* **pull-requests:** read the GitHub mirror from the committed Space settings ([996b74c](https://github.com/develar/branch-deck/commit/996b74c21ea07e719d3805064fadb61677ef39ce))
* push reports its progress, as the fetch does ([cbdc421](https://github.com/develar/branch-deck/commit/cbdc4211f677eaf9c9892b53a9f48bfbd4c0109b))
* read stacked branch prefixes in the grouper, opt-in ([37d6b7a](https://github.com/develar/branch-deck/commit/37d6b7a80e8ae4698b9f9800a6f1eda70b2f6075))
* regroup, rename and restack commits in one validated rewrite ([ed155dc](https://github.com/develar/branch-deck/commit/ed155dc2f742cdedaae16db44e5373052d52e9a1))
* remote commands that accept a non-zero exit code ([d5ce26e](https://github.com/develar/branch-deck/commit/d5ce26e23767bbea50f7b2039bedcfbe13e57771))
* rename and restack commands for the UI ([42fed6c](https://github.com/develar/branch-deck/commit/42fed6c2796f4e71fd3e07cd3520fb61a039eb9b))
* report each branch's place in its stack ([d2da147](https://github.com/develar/branch-deck/commit/d2da147e513d807f9e2187761ab7abd588d1a028))
* retarget a pull request whose base is not the branch's parent ([6bc7c62](https://github.com/develar/branch-deck/commit/6bc7c62cd03bce38177dd08dc7dba4906c4ba0da))
* rewrite history without touching the working tree, require Git 2.54 ([c6c6c79](https://github.com/develar/branch-deck/commit/c6c6c79d3c2890e3f414d9f26368453153f580b5))
* show GitHub pull request status per branch ([6357cc9](https://github.com/develar/branch-deck/commit/6357cc9ac6e45923b5dea98552748060554ecbcd))
* show outgoing commits in the header ([d402090](https://github.com/develar/branch-deck/commit/d402090a813c713d0b47fbe4641abc6c6d5bc207))
* Space preview on branch rows ([a460908](https://github.com/develar/branch-deck/commit/a460908183c5ece9cba0f89cc2f885f0125f471c))
* squash fixup commits ([ee7d884](https://github.com/develar/branch-deck/commit/ee7d88404c79ad854c914f6e377b2e2d19246b41))
* squash selected commits ([28038e6](https://github.com/develar/branch-deck/commit/28038e6018146fa7ef915d6a8e0bd794c07a0043))
* stack-aware Move to… and commit message hint ([4aa5bd8](https://github.com/develar/branch-deck/commit/4aa5bd8ac832cc57786873094fd0a57682275e6c))
* stack, unstack and rename branches from the branch menu ([feee9b2](https://github.com/develar/branch-deck/commit/feee9b2983eac67a834235409ed345f5142c6f4d))
* stacked branches as a tree in the branch table, blocked ones muted ([52c21e6](https://github.com/develar/branch-deck/commit/52c21e6f808030d32b0a728aa62389769158050d))
* start, restart and cancel Patronus robots; ask before a push breaks a Safe Push ([15a10bf](https://github.com/develar/branch-deck/commit/15a10bf8e1b0be87c133227c46e09290362eec3f))
* tell what of your work landed in the baseline, before and after the update ([a5cfa16](https://github.com/develar/branch-deck/commit/a5cfa16b38bb0cdd4c868698553e29bc27fe6305))
* tell when a Safe Push merged, also from the background ([92a6fab](https://github.com/develar/branch-deck/commit/92a6fabc517af003cba5e26e11588c76ccfcb441))
* the archived branch names the commit that merged it ([63b1179](https://github.com/develar/branch-deck/commit/63b1179d3eeb89dc6adc959725773a5fea351fed))
* the copied prompt for a blocked update sends the agent to bd ([1c55df1](https://github.com/develar/branch-deck/commit/1c55df17d940338978411714c9a4c67672b72df3))
* the Force Push description lists the remote commits it drops ([daa13ae](https://github.com/develar/branch-deck/commit/daa13ae8dcb9f983c9c62c231fb055707c1ef044))
* the remote status lists the commits a push drops ([bb196d7](https://github.com/develar/branch-deck/commit/bb196d77bdc0dfc93695d4fec29fe68f51ce03c6))
* the update conflict dialog shows who changed each conflicting file ([f1a169e](https://github.com/develar/branch-deck/commit/f1a169ef3204aaa21c48cdb3c32aa4793a879414))
* trial sync of a regroup plan, with merges memoized by their trees ([32d148d](https://github.com/develar/branch-deck/commit/32d148d01d0c9608a38b3689bd5a6cd230c002d6))
* **ui:** give toasts one vocabulary and open the object they are about ([2fdf172](https://github.com/develar/branch-deck/commit/2fdf172210669c4fc2e1f2252a1f436121c120f6))
* **ui:** inline forms with a visible title, progress, Enter to confirm and the focus back; form commands end with "…" ([d92471e](https://github.com/develar/branch-deck/commit/d92471e66d9843cdccfbc4d4475de99c9156510d))
* **ui:** push button keeps its colour, describes itself when disabled, asks before dropping remote commits ([4302584](https://github.com/develar/branch-deck/commit/4302584faa0225e3f96dc9b03f4a429987b210c4))
* **ui:** Safe Push and "⋯" in the row, Create Merge Request and Create Pull Request ([5ab3ae8](https://github.com/develar/branch-deck/commit/5ab3ae84e9068ab938cb5c6bcead007f8bbab2c7))
* **ui:** shared Callout, InlineCode, HelpPopover and dialog frame widths ([7ed5c4a](https://github.com/develar/branch-deck/commit/7ed5c4ac735802ef6dd01ddcc225f3db04afde08))
* update project onto the upstream without touching unrelated local changes ([ff366ae](https://github.com/develar/branch-deck/commit/ff366ae50af6385c12664dbc118dab40868c07d4))
* Update Project shows the fetch progress, the way IDEA does ([3a0431f](https://github.com/develar/branch-deck/commit/3a0431fc4173c4a24e77d6c256ea9e17777ea4e0))
* **update:** show when an upstream commit was authored and when it landed ([6cda46d](https://github.com/develar/branch-deck/commit/6cda46df214e1d54451ce4af30a437bc3a609719))


### Bug Fixes

* "1 of your commits" in the remote status description ([09d3692](https://github.com/develar/branch-deck/commit/09d36928f6bdc034c26e519e519d3ea15380c91d))
* a plan that keeps an issue commit in its own issue branch rewrites nothing ([91d4d07](https://github.com/develar/branch-deck/commit/91d4d07cdae134a25df4fa8f59f25ae65e802a63))
* a Safe Push started from the form cancels the running one instead of restarting it ([597d7b8](https://github.com/develar/branch-deck/commit/597d7b8d42653c9bc022c354971bdc62a4ff064e))
* a Safe Push that folded fixups into their target no longer makes Update Project conflict ([5e28ac9](https://github.com/develar/branch-deck/commit/5e28ac901caf6b6690d7faad75a5e31fb65f266d))
* a write of the issue groups keeps the keys of other branches, and an undo token is read strictly ([d169ad6](https://github.com/develar/branch-deck/commit/d169ad6f7c3d29900e21f79a20661775bae4eaa3))
* **a11y:** buttons for hover triggers, named repeated labels, one focus ring, data-status ([3ba4895](https://github.com/develar/branch-deck/commit/3ba48950cfdba62ae247dc7a8fe81dc2d718083d))
* AI branch name suggestions name one branch, without a slash ([d5147d2](https://github.com/develar/branch-deck/commit/d5147d2be091b7f503eb12bec1cf975e2ee88813))
* **ai:** plain texts, one chosen-chip style and spinner placeholders in the AI controls ([26465e7](https://github.com/develar/branch-deck/commit/26465e7c8ae592b266dcd9c72061ab9a47b0a60e))
* archive and unapply names avoid ref directory/file conflicts ([d41724a](https://github.com/develar/branch-deck/commit/d41724a3a08be16239c9f6daed87cd7c2fa1805b))
* **cli:** a failure after a bd group rewrite is the error of its output, and descriptions keep what they replace ([56970c3](https://github.com/develar/branch-deck/commit/56970c34ffd2e268f75d14c2125c5e384f8b9d50))
* **cli:** bd suggest hashes its dump with a random SipHash key and repairs conflicts inside issue groups ([8bcb05f](https://github.com/develar/branch-deck/commit/8bcb05f0b5da24d7c529bbaa7bf6ea9ec04922f7))
* **cli:** the brief commits view keeps a stack prefix, and the schema requires no field an output leaves out ([18c5cc4](https://github.com/develar/branch-deck/commit/18c5cc41d40d0ea9aec3daab3cc32cf511e70a53))
* **cli:** the update moves HEAD only under the HEAD lock, the undo is sealed, and a blocked prompt never stashes ([45b36a7](https://github.com/develar/branch-deck/commit/45b36a7789b0aebb1a10144bdede0573b0735e06))
* compare a robot and a pull request with the remote branch, not with the local hash ([9af2329](https://github.com/develar/branch-deck/commit/9af232953ba02156f867b6d5326836b932538f85))
* **conflict:** nested panels, one conflict title and insight, and no empty Diff View ([f86a516](https://github.com/develar/branch-deck/commit/f86a51636ea2076824e730269ef874e0acd07e62))
* **conflict:** pad the commit card of a sub-window inside its body ([197443a](https://github.com/develar/branch-deck/commit/197443ae6f62fb4803c4e6b0ab19f6890a6d0c42))
* don't claim an update state before the first fetch, nor offer "fetch and update" while one runs ([4b6b8de](https://github.com/develar/branch-deck/commit/4b6b8ded15b67ebd7eafe2b41332f714c197919c))
* **e2e:** a commit list is aria-busy while its file counts load, and HTML snapshots wait for it ([6f27b06](https://github.com/develar/branch-deck/commit/6f27b062c6566b26c805aa921c2463b3d599802f))
* **e2e:** a never-fetched baseline says it was not fetched yet ([b2389e8](https://github.com/develar/branch-deck/commit/b2389e8e99b2723ac3812882cdf760e01f0cc6ae))
* **e2e:** an HTML snapshot waits for closing popups, not for collapsed accordions ([d8467a9](https://github.com/develar/branch-deck/commit/d8467a912eb5f3dcf76870bc23612936b84a7b14))
* **e2e:** close the push description after a push from the row ([87e6321](https://github.com/develar/branch-deck/commit/87e6321884960cdfafc122ca44875de53bd64d60))
* **e2e:** confirm the force push that drops a remote commit in the Activity test ([e16616c](https://github.com/develar/branch-deck/commit/e16616cd0a328bca44ecc5d911aba6517bfb01f3))
* **e2e:** hover the common ancestor hash once the unassigned commits are above the table ([634a44d](https://github.com/develar/branch-deck/commit/634a44da3690f66e120bbbfc7ca38dc1af681239))
* **e2e:** keep the times of the update templates before the page clock ([56e875b](https://github.com/develar/branch-deck/commit/56e875bdff0ec2e6c17561d35c98bb63a39060c3))
* **e2e:** move the pointer off the push button after a push from the row ([abe9ac5](https://github.com/develar/branch-deck/commit/abe9ac5290b3ce09faf8e190e1665ced63e077cd))
* **e2e:** press the squash shortcut once the message editor has the focus ([b329c45](https://github.com/develar/branch-deck/commit/b329c4544eea826a166171a1de6201d80fc971dc))
* **e2e:** read the open fixup tooltip, not the closing one of the expand button ([03f74e3](https://github.com/develar/branch-deck/commit/03f74e3af64c8e607323ed2120f9094e4215e0d2))
* **e2e:** read the title of the commit preview as its level-2 heading ([4512738](https://github.com/develar/branch-deck/commit/4512738ebba96a3f6eb3b4551b50b83c247421bd))
* **e2e:** the merged-by line of an archived branch has no time ([f4e47ee](https://github.com/develar/branch-deck/commit/f4e47eef6196a845d69bd60be98cb74e7691c4c9))
* **e2e:** the Restart of the robot details is named "Restart Safe Push" ([8038044](https://github.com/develar/branch-deck/commit/803804415c22d677652da03fdfb8cd5507dd997b))
* **e2e:** the simple template's test-branch has one commit ([d6dd95f](https://github.com/develar/branch-deck/commit/d6dd95f00220baf35fb28e09d09b4e870aac77d8))
* **e2e:** wait for the merged status word before expanding an archived branch ([769f793](https://github.com/develar/branch-deck/commit/769f7937f1370c1aeeb8d3fd905ef122b0e89ae1))
* **e2e:** wait for the robots and for the push guard's focus in the Patronus actions ([a6b1fd5](https://github.com/develar/branch-deck/commit/a6b1fd53183152b6b20dd6cb670a263bc8a2efac))
* **git:** a dropped interactive git exits before it is killed, so it leaves no ref locks ([02a78b5](https://github.com/develar/branch-deck/commit/02a78b5ef36167327a519e3b8209bab1aa63f531))
* **git:** an update refuses a HEAD switched to another branch since the check ([59a34b4](https://github.com/develar/branch-deck/commit/59a34b44984f8a5fb1e87118c2de4d4bd56b1505))
* **git:** mapping notes are read from their own notes ref only ([ef230b8](https://github.com/develar/branch-deck/commit/ef230b805112b2c58ffdd52cbb8eb41ccdfb3339))
* give each landed number its verb, in one grammar on every surface ([c7d7a92](https://github.com/develar/branch-deck/commit/c7d7a92d10fba453bb7a6827e877619adac095dd))
* **header:** keep the header on one line from 960 px and still during a sync ([5f18c89](https://github.com/develar/branch-deck/commit/5f18c89278854a8d4ae4100389fb6b910df3831c))
* keep the attention for a failed Safe Push after a sync rebuilt the branch ([4b552ee](https://github.com/develar/branch-deck/commit/4b552ee2223d1526b38e5b56ec50a80d54d61828))
* keep the reword error the UI shows for a commit that left the branch ([dcd299b](https://github.com/develar/branch-deck/commit/dcd299b52a543f71dacd42eaeb7b88f9a6617721))
* one colour rule for push states ([4e3455c](https://github.com/develar/branch-deck/commit/4e3455c801d8deb8cb319e17a8722348d30d049b))
* outgoing counts the work the remote lacks, not commit hashes ([54d9edd](https://github.com/develar/branch-deck/commit/54d9edd2e812e709aca14f8f383420f9ad08afc7))
* **outgoing:** the review chips of the overview never overlap the state word ([37ead8a](https://github.com/develar/branch-deck/commit/37ead8a0a25b5c49abf4ebf54c53eba352a54947))
* **patronus:** a start right after a push is not asked again ([3930b8b](https://github.com/develar/branch-deck/commit/3930b8b12e83f8b3066e1c39dd0b9f4789ef8c87))
* **patronus:** say what a dry run leaves out and which commit the start form pushes first ([8658933](https://github.com/develar/branch-deck/commit/8658933626e48b56f460ca31c28da053b4053798))
* **patronus:** the review button names Space, and the block says why ([d9343dc](https://github.com/develar/branch-deck/commit/d9343dc454c9f45b6915a961d65c40b88587147f))
* push replaces the remote branch only as the row showed it ([992485e](https://github.com/develar/branch-deck/commit/992485ecd1798cacc5f461404372821e63b2f990))
* **push:** count a push once and say what it does ([e06fa79](https://github.com/develar/branch-deck/commit/e06fa79b3a82ee517a07085111483f63057c76a0))
* **push:** keep the user's credential helpers, never prompt ([2b689a8](https://github.com/develar/branch-deck/commit/2b689a821841fb753b6c44bed280fb1d4402ab1a))
* relative times age while they are shown ([0be537d](https://github.com/develar/branch-deck/commit/0be537db1227c92b765818eae54da370994c4ba9))
* reuse a virtual commit only on its parent and write deterministic copies ([89960d7](https://github.com/develar/branch-deck/commit/89960d7f79133bd7ba807333d29626f99f357d17))
* show the copy buttons of the baseline and merged-by commits on hover ([c06c84e](https://github.com/develar/branch-deck/commit/c06c84ebf0a63c709a65583b6ee2f5a42aabfaa5))
* show the issue links of commit subjects in compact lists ([8f0a0be](https://github.com/develar/branch-deck/commit/8f0a0bef2baabbea3b6d7a1ce84c7d0ab50b453d))
* **stacks:** the first sync after a parent's merge moves its child ([543c2b2](https://github.com/develar/branch-deck/commit/543c2b2d523432a0387816e8ac61977db9b3f3b3))
* test server serves only the dist of its own workspace ([1ea6c1f](https://github.com/develar/branch-deck/commit/1ea6c1f51a5c6cd8cb67cf492aa5da7282db729b))
* the outgoing overview footer names the update state as incoming, with the fetch time ([df4aeda](https://github.com/develar/branch-deck/commit/df4aedadb2ec56befeb43e245e05273fe51db166))
* the Update button tooltip names the running step, not the last state ([9fcd46b](https://github.com/develar/branch-deck/commit/9fcd46b87146e27b8923ba9f14b9d6bc97f171b7))
* the update step and fetch progress belong to their repository ([10a84f8](https://github.com/develar/branch-deck/commit/10a84f8b28347af494f3bc3a31a77d7bdc95462d))
* **ui:** align the branch row status cell, the chip anatomy and the tree spine ([9cb4f36](https://github.com/develar/branch-deck/commit/9cb4f36e4ea46ab543d481095bea6e4ef3f7f308))
* **ui:** one icon per concept and tooltips on icon-only buttons ([543c89c](https://github.com/develar/branch-deck/commit/543c89c49c690ca62655047d733468f517ab6661))
* **ui:** one meaning per colour ([eb18100](https://github.com/develar/branch-deck/commit/eb18100d300ccb5349f9a504e15103035b53de4e))
* **ui:** text in tooltips, rich content in popovers of one width ([1e77fdb](https://github.com/develar/branch-deck/commit/1e77fdb3d2f3b5cf0e40f09a8513f59d96992906))
* **ui:** the Activity button has a tooltip like every icon-only button ([3b70c07](https://github.com/develar/branch-deck/commit/3b70c073b0ba2ad348845a3eca9b997a499d1c67))
* **ui:** the archived commits load error is a text-sm line like the other states ([dd51ebf](https://github.com/develar/branch-deck/commit/dd51ebf502b577dea8552c15ae185f5c26b2088d))
* **ui:** the branch menu names the Space merge request as the review block does ([56e4711](https://github.com/develar/branch-deck/commit/56e4711477d1dc4855a2202aaff15e517d79b13b))
* **ui:** the path error says its message once, and the welcome steps count from 1 ([aa7ef05](https://github.com/develar/branch-deck/commit/aa7ef054f9723a6b04503c687295031f181cca6b))
* **ui:** type sizes from the scale, tabular live counters, mono hashes ([1229bcb](https://github.com/develar/branch-deck/commit/1229bcbfe97ab9dcd367d053f935c419b48d176e))
* virtual commits keep the original timezones ([bebbd57](https://github.com/develar/branch-deck/commit/bebbd57fa5ec0232df2fb15685db945507a84274))


### Performance Improvements

* bundle common highlight.js grammars instead of all ([0137984](https://github.com/develar/branch-deck/commit/01379842b38c20096d20faa512363be7d3fb571e))
* carry mapping notes across rewords and bound the reword walk ([7beff05](https://github.com/develar/branch-deck/commit/7beff0500388a3ac02379a5f4ab67bb314d0ce1e))
* cheap update check during sync ([12ed3e8](https://github.com/develar/branch-deck/commit/12ed3e87fd482daf5399471d68a49818745d859f))
* e2e in about a minute instead of five ([203b23e](https://github.com/develar/branch-deck/commit/203b23e6cb36130865ff6ee557f168695733433f))
* one ref listing and one notes write per sync ([92e7594](https://github.com/develar/branch-deck/commit/92e7594e712c9eb6d6933df9438172332a1d3500))

## [0.11.0](https://github.com/develar/branch-deck/compare/v0.10.0...v0.11.0) (2026-10-13)


### Features

* commit preview opened with Space instead of the inline body ([dd4ec04](https://github.com/develar/branch-deck/commit/dd4ec04df4fd9b571d1577303d47b4f9ff2d38e6))
* edit a commit message in the commit preview ([24987bc](https://github.com/develar/branch-deck/commit/24987bc3f071c538826f3e7fa4dfc56bdda3d075))
* inline commit message bodies with reading typography ([879f58b](https://github.com/develar/branch-deck/commit/879f58bdd895ea52f0e57be99d2c21ec1fe37f68))
* **model:** switch to Qwen3-4B-Instruct-2507 and remove obsolete model caches ([b6b55a7](https://github.com/develar/branch-deck/commit/b6b55a780c61e31c56de71a3e34715b15f7b4a9a))
* move commits between branches, split an issue branch into pushable branches ([45a889e](https://github.com/develar/branch-deck/commit/45a889edaab4309766096fbab8d4e0a90dd3b8e6))
* one commit row anatomy, quieter branch table and conflict viewer, wider body ([11c6a17](https://github.com/develar/branch-deck/commit/11c6a17e2f46db06e2f9ac8ed871d9f396362601))
* **react:** auto-update, menu sync, auto-sync on focus and theme dialog ([7b28a3d](https://github.com/develar/branch-deck/commit/7b28a3deedd92d918f80246f51abf743a543d16c))
* **react:** branch sync store, branch tables and inline row forms ([9ed746f](https://github.com/develar/branch-deck/commit/9ed746f4bdc0131f6207f7714b878e98e178b2c1))
* **react:** commit list, row selection and inline branch creator ([7ffc7ba](https://github.com/develar/branch-deck/commit/7ffc7ba1597314f10b0c3cdc01d232131f3300c3))
* **react:** conflict viewer and AI branch naming ([1e52bbf](https://github.com/develar/branch-deck/commit/1e52bbf1f1de842cde9da0c1ac389715c11d7342))
* **react:** core infrastructure, settings stores and app shell ([8452005](https://github.com/develar/branch-deck/commit/84520058d4fa1879d203a925b78879d4eb564fc6))
* **react:** repository selection, configuration header and welcome card ([3f9bd6f](https://github.com/develar/branch-deck/commit/3f9bd6fc1ba11308afc68884243761fd53a8b837))
* readable commit message popover with reflow and scrolling ([50177f4](https://github.com/develar/branch-deck/commit/50177f40646aa719017dca293b5e30b852fecf00))


### Bug Fixes

* **amend:** restore the working tree after an aborted autosquash rebase ([18a66b4](https://github.com/develar/branch-deck/commit/18a66b424816054d0585fcb1afc337add3086b97))
* dedupe tokenizers and apply toast theme override ([135807c](https://github.com/develar/branch-deck/commit/135807c172626b0ed46072fae6df82a18305b4b4))
* give the commit title the free width of the branch table ([5c55331](https://github.com/develar/branch-deck/commit/5c55331e426deb82b8cbcd325d1a0a67f42bfd7d)), closes [#72](https://github.com/develar/branch-deck/issues/72)
* history rewrites move the checked out branch only if it did not move ([698ceb5](https://github.com/develar/branch-deck/commit/698ceb599c3e18dd851cca4282392c267dca88a3))
* keep archived branches newest first ([9b9bc33](https://github.com/develar/branch-deck/commit/9b9bc33fd86b48d1afeaf286950e0195da307814))
* keep the push state of a branch across a sync ([7fafdfb](https://github.com/develar/branch-deck/commit/7fafdfbf9a971c1f2d080782d0b8c87204529eeb)), closes [#71](https://github.com/develar/branch-deck/issues/71)
* **model:** keep the partial download when the user pauses ([61a38f5](https://github.com/develar/branch-deck/commit/61a38f5d18895976fce674e24b038957a9dfbb01))
* sync, archive and model bugs found in Rust review ([b17a407](https://github.com/develar/branch-deck/commit/b17a407cb6687d5b42d4fee0fb6016114f3f8211))

## [0.10.0](https://github.com/develar/branch-deck/compare/v0.9.2...v0.10.0) (2025-10-28)


### Features

* add auto-sync on window focus ([fd18d51](https://github.com/develar/branch-deck/commit/fd18d51fa9dfab92ada0c6a4328b32320fe22921)), closes [#45](https://github.com/develar/branch-deck/issues/45)
* amend local changes to a branch ([2ceb6a2](https://github.com/develar/branch-deck/commit/2ceb6a24197f47ab80bde04323ae65c8b57c6ec9))
* support fixup! commits ([9b64d85](https://github.com/develar/branch-deck/commit/9b64d85885ff89491aee8d7361ba7232e1963868)), closes [#51](https://github.com/develar/branch-deck/issues/51)

## [0.9.2](https://github.com/develar/branch-deck/compare/v0.9.1...v0.9.2) (2025-09-11)


### Bug Fixes

* remote status - use path-id based strategy ([8b3292a](https://github.com/develar/branch-deck/commit/8b3292acd0de85d4fac7bd67329ea76d1fc9e04c))

## [0.9.1](https://github.com/develar/branch-deck/compare/v0.9.0...v0.9.1) (2025-09-09)


### Bug Fixes

* inline action progress ([6f89681](https://github.com/develar/branch-deck/commit/6f89681c1c00bc90b4ac014686c65a98bd493c49))

## [0.9.0](https://github.com/develar/branch-deck/compare/v0.8.2...v0.9.0) (2025-09-08)


### Features

* amend local changes to a branch ([fd2ad77](https://github.com/develar/branch-deck/commit/fd2ad7759f41f5dba914e29dc5959964db7fd915))

## [0.8.2](https://github.com/develar/branch-deck/compare/v0.8.1...v0.8.2) (2025-09-04)


### Bug Fixes

* update remote sync on push ([dd7ad83](https://github.com/develar/branch-deck/commit/dd7ad8352b723744ab6da55435bd7337830a10ef))

## [0.8.1](https://github.com/develar/branch-deck/compare/v0.8.0...v0.8.1) (2025-09-01)


### Bug Fixes

* disable add issue reference action if all commits have issue number ([d6c4748](https://github.com/develar/branch-deck/commit/d6c47489c704795f0c595f8f8a3a8f7508416edc))

## [0.8.0](https://github.com/develar/branch-deck/compare/v0.7.1...v0.8.0) (2025-08-25)


### Features

* integration status ([5ce7e7b](https://github.com/develar/branch-deck/commit/5ce7e7b9de4443b5fd2e9dc5aa82d84f40b8d1eb))

## [0.7.1](https://github.com/develar/branch-deck/compare/v0.7.0...v0.7.1) (2025-08-07)


### Bug Fixes

* make AI feature more visible ([ec22728](https://github.com/develar/branch-deck/commit/ec22728dff6bfa28ecf9bc17d58ded8c46278011))

## [0.7.0](https://github.com/develar/branch-deck/compare/v0.6.0...v0.7.0) (2025-08-02)


### Features

* unassigned commits - AI branch name generation ([a6324ff](https://github.com/develar/branch-deck/commit/a6324ff9db665e285781b0a83b196de618ab5c06))

## [0.6.0](https://github.com/develar/branch-deck/compare/v0.5.0...v0.6.0) (2025-07-16)


### Features

* unassigned commits ([52a9c7d](https://github.com/develar/branch-deck/commit/52a9c7da384cc30a5befa7820581024eb362daa0))

## [0.5.0](https://github.com/develar/branch-deck/compare/v0.4.2...v0.5.0) (2025-07-14)


### Features

* merge conflict viewer ([3ce1b34](https://github.com/develar/branch-deck/commit/3ce1b34dd537f338f6fb67d143e93dc43025e69e))
* merge conflict viewer (fix sub windows) ([fdc13b3](https://github.com/develar/branch-deck/commit/fdc13b3dc5954d310944dcf59a2b2bb69fc102c2))

## [0.4.2](https://github.com/develar/branch-deck/compare/v0.4.1...v0.4.2) (2025-07-08)


### Bug Fixes

* case-insensitive git config ([31e39d4](https://github.com/develar/branch-deck/commit/31e39d430af171be3c02f63502f11093ebaf12c3))
* move auto-update logic from js to rust to improve security ([b937316](https://github.com/develar/branch-deck/commit/b937316ee19729f90bbe8209de63d6bba40f73fe))
* use `()` instead of `[]` as prefix because `[]` is a common convention for subsystems ([8a78518](https://github.com/develar/branch-deck/commit/8a78518cb4a045b882d8459c1b09729c09b6c004))

## [0.4.1](https://github.com/develar/branch-deck/compare/v0.4.0...v0.4.1) (2025-07-07)


### Bug Fixes

* relaunch after auto-update ([da5ec34](https://github.com/develar/branch-deck/commit/da5ec34bcde63b7a8eb774ad2dfd9d73e6a139da))


### Performance Improvements

* merge using tree, no need to use commits ([4ba8048](https://github.com/develar/branch-deck/commit/4ba8048cca2d1797380f50ece1885b6f14600a4e))

## [0.4.0](https://github.com/develar/branch-deck/compare/branch-deck-v0.3.0...branch-deck-v0.4.0) (2025-07-06)


### Features

* branch parallel processing, sanitize branch name ([8821954](https://github.com/develar/branch-deck/commit/8821954bae9f1aad67c72879af588dce6bd9894e))


### Bug Fixes

* slow cherry-pick ([252f076](https://github.com/develar/branch-deck/commit/252f07683fd0bd7ed29b520a7324f7de1e8992f9))
* truncate commit message text ([a9bcecf](https://github.com/develar/branch-deck/commit/a9bcecf4c9e789e00a9d96e532cd5d5ae6e8c37e))
* use a correct base commit to avoid cherry-picking unrelated changes ([372ebe7](https://github.com/develar/branch-deck/commit/372ebe7bc4a6e4f03d9da0bd18a3d7efa3ede640))

## [0.3.0](https://github.com/develar/branch-deck/compare/branch-deck-v0.2.0...branch-deck-v0.3.0) (2025-07-06)


### Features

* auto-update ([00ab5ef](https://github.com/develar/branch-deck/commit/00ab5efc56384520a83678c5264ea5682ec95659))


### Bug Fixes

* truncate commit message text ([a9bcecf](https://github.com/develar/branch-deck/commit/a9bcecf4c9e789e00a9d96e532cd5d5ae6e8c37e))

## [0.2.0](https://github.com/develar/branch-deck/compare/branch-deck-v0.1.0...branch-deck-v0.2.0) (2025-07-02)


### Features

* branch parallel processing, sanitize branch name ([8821954](https://github.com/develar/branch-deck/commit/8821954bae9f1aad67c72879af588dce6bd9894e))


### Bug Fixes

* use a correct base commit to avoid cherry-picking unrelated changes ([372ebe7](https://github.com/develar/branch-deck/commit/372ebe7bc4a6e4f03d9da0bd18a3d7efa3ede640))
