## MODIFIED Requirements

### Requirement: One window: Every review is one window whose data says what it shows

Every review MUST be one window, `review` (`/review`, `ReviewPage`), whose data says what it shows
(`ReviewWindowData`): a branch with its stack ancestors, the whole stack of a root or a kept branch, a commit of a
list, a pull request waiting for the user's review, or a stack of them.

#### Scenario: A branch, the whole stack of a root or a kept branch
- **WHEN** the window shows a branch with its stack ancestors, the whole stack of a root or a kept branch
- **THEN** its data is `kind: "branch"`, its chain with `wholeStack` or `kept`

#### Scenario: A commit of a list
- **WHEN** the window shows a commit of a list
- **THEN** its data is `kind: "commits"`: the commits of the list as the opener had them, its name, the commit, the file
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review an unassigned commit in the review window of the commit`)

#### Scenario: A branch and then a commit of a list in the same window
- **GIVEN** the window shows a branch
- **WHEN** a commit of the unassigned list is reviewed
- **THEN** the same window shows the commit of the list
  [@test] tests/e2e/review-window/review-window.spec.ts (`should review a branch and then a commit of the unassigned list in the same window, and go back with ⌘[ and with Back`)

#### Scenario: A pull request waiting for the user's review
- **WHEN** the window shows a pull request waiting for the user's review
- **THEN** its data is `kind: "pullRequest"`: the request, its place in Your Turn or Reviews, the write the row asked
  for ([Review Requests](../../repository/review-requests/spec.md#review-window))

#### Scenario: A stack of pull requests
- **WHEN** the window shows a stack of requests of Your Turn (Review Stack)
- **THEN** its data is `kind: "pullRequestStack"`: its top, its bottom, its layers when it opened, and "review" for `D`
  ([Review Requests](../../repository/review-requests/spec.md#requirement-review-window-review-stack-reviews-a-stack-in-one-window))
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open one window of a stack by the button, the menu and D of its header`)

### Requirement: One window: The review of a pull request is a chunk of its own

The review of a pull request, and of a stack of them, MUST be a chunk of its own that the page loads only for it, so
the first frame of the others keeps its size.

#### Scenario: The window shows no pull request
- **WHEN** the window shows a branch or a list of commits
- **THEN** the page does not load the chunk of the review of a pull request, and its first frame keeps its size

### Requirement: One window: One rule says how an open goes on from the review on screen

How an open goes on from the review on screen MUST be one rule for every subject (`planReviewOpen`): an open of the
subject on screen only brings the window to the front, where the review is; a commit of it moves there (`move`);
anything else starts the window over (`startedAt`). The page of the window MUST apply it to every open it gets, from
any window (`takeOpen`): an opener only says what to show.

#### Scenario: A pull request
- **WHEN** the window shows a pull request and gets an open
- **THEN** a plain open of the request it shows keeps its review
- **AND** another request or an intent starts it over
  [@test] src/lib/review-reuse.test.ts (`keeps its review for a plain open of the request it shows, and starts over for another request or an intent`)

#### Scenario: A stack of pull requests
- **WHEN** the window shows a stack and gets an open
- **THEN** a plain open of the stack of the same top keeps its review
- **AND** another stack, `D` on its header or a pull request starts it over
  [@test] src/lib/review-reuse.test.ts (`brings the stack on screen to the front for a plain open of it, and starts over for another stack, D or a pull request`, `names a stack by its bottom and its top`)

#### Scenario: A branch
- **WHEN** the window shows a branch and gets an open
- **THEN** a plain open of the branch it shows keeps its review
- **AND** another branch or a place starts it over
  [@test] src/lib/review-reuse.test.ts (`keeps its review for a plain open of the branch it shows, and starts over for another branch or a place`)

#### Scenario: A commit of a branch of the chain
- **WHEN** the window shows a chain and gets an open of a commit
- **THEN** a commit of a branch of the chain on screen moves there (`move`)
- **AND** a commit of another branch starts it over
  [@test] src/lib/review-reuse.test.ts (`moves to a commit of a branch of the chain on screen, and starts over for a commit of another branch`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain`)

#### Scenario: A kept branch and a whole stack
- **WHEN** the window shows a kept branch or a whole stack and gets an open
- **THEN** a plain open of it keeps it
- **AND** a commit of the kept branch moves there
  [@test] src/lib/review-reuse.test.ts (`keeps a kept branch and a whole stack for a plain open of them, moves to a commit of the kept branch`)

#### Scenario: A list of commits
- **WHEN** the window shows a list of commits and gets an open
- **THEN** a commit of the list on screen moves there
- **AND** another list, a file or a branch starts it over
  [@test] src/lib/review-reuse.test.ts (`moves to a commit of the list on screen, and starts over for another list, a file or a branch`)

#### Scenario: Anything else starts the window over
- **WHEN** the open is another subject, a place such as a file of a list, a reviewer group, the whole stack of a
  branch, another pull request or a write of one, or another repository
- **THEN** the window starts over (`startedAt`)
  [@test] src/lib/review-reuse.test.ts (`keeps the review on screen for a plain open of it, moves to a commit of it, and starts over for anything else`)
