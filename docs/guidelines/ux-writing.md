# UX Writing: Toasts, Errors and Labels

**Status:** Active  
**Updated:** 2026-10-25  
**Tags:** [ui, wording, toasts, actions]

## Summary

One vocabulary for the whole app. A toast says what happened, to what, in sentence case; its description is the detail;
an error says what could not be done and why; a toast about an object opens it. Buttons, menu items and toast actions
are Title Case; everything that reads as a sentence is sentence case. [Activity](../../spec/repository/activity.spec.md)
records the toasts in their words, so a toast and its entry take their text from one function. A toast is one per
event of an object: its later states replace it in place, and its actions are those of the state now. An object's
actions are one list, under one name each, wherever the object is.

## Context

Before this guide the toasts spoke three dialects: "Success / Branch X pushed successfully", "Pushed oauth" and
"`feature-auth`: Changes Amended"; errors were "Failed to …", "… Failed", "Can't …" and "Error"; some error toasts were
sticky and logged, others vanished after 5 s. The user read a toast, then could not find the thing it was about.

## Toasts

- **The title says what happened**, in the past tense, with its object: "Pushed feature-ui", "Amended 3 files into
  feature-auth", "Updated master: rebased 2 commits onto origin/master", "Safe Push started", "auth merged into master".
- **Never "Success" or "Error" as a title**: the icon already says it.
- **The description is the detail**: what the push did ("1 commit · created the remote branch", "1 commit · dropped 2
  remote commits"), what a robot tests ("air-session-runtime → master · profile Large Scale Change"), the amended commit.
  It never repeats the title.
- **No "branch: Title Case" prefixes** ("feature-auth: Changes Amended"): the branch is part of the sentence ("Unapplied
  feature-auth").
- **Name the object the user knows**: the virtual branch, never its ref (`develar/virtual/air-session-runtime`).
- **An issue key is a link**: the toast renders the issue keys of a text title and description as links to the tracker
  (`LinkedText` with the issue navigation of the project, `renderText` of the `Toaster` in `App.tsx`), as the rows do; a
  caller passes plain text and never builds the link.
- **A toast about an object opens it** with its action:
  - a Patronus robot (started, restarted, canceling, merged): "Open in Patronus" (`robot.url`);
  - a pushed branch: "Open Pull Request", or "Create Pull Request…" without one (GitHub; the dialog asks for its text
    and reviewers);
  - a created pull request: "Open Pull Request";
  - a Space merge request: "Open Merge Request";
  - the installed Agent CLI: "Copy Path".
  A toast with an action stays 10 s (`ACTION_TOAST_TIMEOUT_MS`). An action that asks for more ends with "…" (below)
  and opens a dialog or a popover: its toast closes when it is pressed (`opensDialog` of
  `src/components/ui/toast.tsx`, for every toast and both of its actions), since a toast lies above the dialogs and
  would cover the footer of the very dialog it opened until it timed out; tests waited 9 s for one there. A toast with a description and an action, or with two
  actions (the merged robot: Fetch, then "Open in Patronus", `data.secondaryAction` of `src/components/ui/toast.tsx`),
  has its actions below the text, the primary one first: beside it, a button squeezes the title into a narrow column.
  A toast of a title alone keeps its action beside it.
- **A precondition is `info`, not an error**: "No commits to unapply", "No changes to amend", "Nothing to change",
  "master is already up to date with origin/master". Nothing failed; nothing is logged.
- **No toast for feedback in place**: a copy button says "Copied" in place (`CopyButton`, `useCopyToClipboard`), the
  "Copy Prompt for Agent" button too; a menu item copies without a toast; an amend conflict opens its viewer below the
  row and says nothing else.

## One toast per event

A robot merged auth: "auth merged into master" with Update Project. The user pressed it, and the update said "auth
landed in origin/master, squashed" in a second toast, while the first stayed with an Update Project already done: one
landing told twice, in two words, one of them with a stale action. A toast tells an event of an object, and its later
states are that toast, not new ones.

- **One toast per event of an object**: a toast is keyed by what it tells (`toastKeys` in `src/lib/event-toast.ts`):
  the robot, the robots of a subject in one push mode, the push of a branch, the approvals of a subject, a pull
  request of someone else, an issue, the update of a repository. A later state of the event replaces the open toast
  of its key in place, where it is, with its timer started again: "auth merged into master" becomes "Updating…" while
  the update runs, then "auth landed in origin/master, squashed" / "Updated master: rebased 104 commits". A state
  that supersedes another event names that one's key too: the outcome of an update takes the place of the merges of
  its repository it brought in, a restart or a new start the toast of the older robot of the subject, a Safe Push the
  toast of the approvals it starts on, a review the toast of the request, the start of Approve and Safe Push the
  toast of its approval, a push of a branch the toast of its earlier push. Several toasts it supersedes become one:
  the update that lands three merges is one toast, "3 branches landed in origin/master", in the place of the newest.
- **Its actions are those of the state now**: the toast renders its state from the stores it reads, at the show and
  on every change of them. Update Project leaves once the update landed the merge; it is "Updating…", disabled, while
  an update runs, started from the toast, the header or a key, and the toast stays until its outcome; Cancel Safe Push
  leaves once the robot finished; a restart made elsewhere turns the toast into "Safe Push of auth was restarted" with
  the newer robot; "Create Pull Request…" becomes "Open Pull Request" once the branch has one. A click runs the action
  of the latest state, never one decided when the toast showed.
- **A toast outlives no object**: an action of a branch of the table leaves once the main window shows another
  repository, whose branch of the same name is another branch (the push, the start, the pull request: Fetch instead of
  Update Project, the toast of a remote that moved without its push); a toast whose object is gone closes (the remote
  that moved of a deleted branch, the review requests none of which waits).
- **A state that is no news of its own only replaces**: a landing a sync found after an update elsewhere (`bd update`,
  the IDE) says itself in the toast of its merge while that one is open ("auth landed in origin/master" / "Found by a
  sync") and shows nothing otherwise; so does a failure told failing before ("A required check of auth failed" becomes
  "Safe Push of auth failed").
- **A run without an outcome of its own closes it**: a toast whose action ran (Updating…) closes once the run ended
  with a dialog or an error toast that says why (the update conflicts, the fetch of another repository).
- **Activity keeps every entry**: it is the log of the events, each in the words its toast had then; only the toasts
  are one per event.

## Errors

- **Every error goes through `notifyError(title, error)`** (`src/lib/errors.ts`): the toast is sticky, the error is
  logged to the backend log and recorded in Activity. Never a hand-made `toast.add({ type: "error" })`;
  `withRowProcessing` and `runRewrite` take an `errorTitle` and call `notifyError` themselves. The same error while its
  toast is open updates that toast, never a second copy.
- **The title is "Cannot <verb> <object>"**: "Cannot push feature-ui", "Cannot start the Safe Push", "Cannot copy to the
  clipboard". One failure verb: not "Failed to …", "… Failed", "Can't …", "Couldn't …".
- **The description is the reason**, the `message` of the error (`getErrorDetails`), never prefixed with "Error:".
- The same words in a tooltip or an inline error: "Cannot check the update: <reason>".
- **A read nobody asked for is a state, not an error**: a view that reads as it shows (the Landing view's statistics)
  says its failure in the view, as its state ("Cannot read the robots", kept numbers marked as of an earlier read),
  without a toast; Rust logs it. The same read asked for (Refresh) goes through `notifyError`.
- **Typing a field refuses says what is wrong**, under the field, without "Cannot …": nothing was attempted ("A prefix
  cannot contain spaces, …", "acme/app/x is not a GitHub repository: use owner/name").

## Help

- **A help popover is read in five seconds**: its title is the name of what it explains, then what it is and what the
  app does with it, in 1–3 short sentences; the keys last. No "automatically" (everything the app does is), no
  benefit talk ("to keep your repository clean"), no muted footnotes, no weight on a word of the content.
- **True to the code**: a number names its constant in a comment beside the text (`ARCHIVE_RETENTION_DAYS`), a key is
  shown only where its handler works.
- **Help repeats no form**: what a form does belongs in its description ("Amends the uncommitted changes to the last
  commit of X, keeping its message and author."), not in a popover beside it.

## Words and punctuation

- **Title Case** for buttons, menu items, card titles, tabs and toast actions: "Create Pull Request", "Open in
  Patronus", "Copy Path", "Install and Restart".
- **Sentence case** for dialog titles, toast titles, descriptions, tooltips, placeholders and help heads, and for the
  label of a switch and the heading of a settings section ("Review requests", "Count failed robots and conflicts",
  "Dock icon"): a switch states a setting, it is not a command. A switch that mirrors a menu item keeps the item's
  Title Case ("Auto-sync on Focus").
- **"…"** (the ellipsis character, never "...") for progress ("Pushing…") and on every command that asks for input
  before it acts ("Unapply…", "Create Pull Request…", "Safe Push With Profile…"). An action that acts at once, or asks
  only to confirm what it has to say first, has none: "Safe Push" starts, or opens a popover only when it cancels a
  running robot, lands more than its row or runs another profile (as macOS: the ellipsis means more input, not a
  confirmation).
- **"canceled"**, "canceling" (US spelling), "Branch Deck".
- **No exclamation marks**.
- **"·" is the only separator** between parts of a line ("3 commits · created the remote branch"), also between the
  items of a line that come from several places (the island header); never "•", "|" or " - ".
- **A list of names in a sentence** names up to four ("auth, session, login and oauth") and from five on the first
  three and the count ("auth, login, oauth and 2 more", `joinNames`): never "and 1 more", where one name would do.
  Every cut list follows the same rule, a dialog's, a prompt's for an agent ("… and 280 more files"), a hover's
  ("Failed: Fast Tests, Smoke, Compilation and Lint", four of four), a chip row's ("+2 branches"), the dropped
  commits of a push ("and 2 more" past ten) and `bd`'s output (`files_omitted`, " (2 more: bd check)" after the
  dangling references it describes): a list one longer than its limit is given whole, the count of the rest is never 1
  (`cutList`; the backend's lists `sync_utils::list_cut::kept`). An item with no name to stand there is said as such,
  never counted alone ("tests and a check without a name", a GitHub check without one), and a text cut to a budget
  sends its one line past it rather than "1 more lines" (the input of the branch naming, "[… 2 more lines omitted]").
  The one exception is a list whose rows a poll keeps
  in place: Your Turn shows three rows, then "Show 1 More", since a fourth row would come and go with the polls.

## Actions

The same object (a branch, a Patronus robot, a pull request) appears in a menu, a popover, a footer and a header list.
Built by hand in each place, the actions drifted apart: the menu named a restart one way, the popover another; the branch menu showed `O` and `M`, which worked only in the preview or with a selection; the robots
indicator could cancel a robot but not restart it.

- **One builder per object**: the actions of an object come from one function returning `MenuItem`s, each with a
  stable `id` (`getRobotActions` in `src/lib/patronus-actions.ts`, `getBranchContextMenuItems` in
  `src/hooks/useBranchContextActions.ts`). A menu renders the list (`MenuItems`); a popover, a footer or an icon row
  renders the same items as buttons, picking them by `id`. A surface never builds its own copy of an action.
- **A menu leads with what lands its object**: the branch menu starts with "Safe Push", then the push and the review;
  the rarer forms of an action go into a submenu named by what they belong to ("Patronus ›": Safe Push With Profile…,
  Dry Run; "Copy ›": the names and links). An item with a key stays on top, where its hint is seen. The branch menu
  once listed 19 items in 7 groups, the copies first and "Park Branch…" last, alone in its group.
- **Object actions stay in every form**: a compact form (a row, a popover) and an expanded one (a preview, a window)
  offer the same actions of the object; only the view modes (Review, Files) change with the form.
- **A key hint only where the key works**: an item shows its key (`kbds`, `<Kbd>`) only on a surface whose handler
  has the key; a key shown in the menu works on the focused row too.
- **What a text names opens from it**: a hover, a notice or a toast that names an object or a move offers it right
  there, from the builder of the object: an issue key is a link to the tracker, "Safe Push" of a robot opens it in
  Patronus, "#12" its pull request, "Blocked by auth" offers "Show auth", "…or Restart Safe Push" the restart. A
  tooltip holds no link (the local `Tooltip` is not hoverable): a hover that names one is a popover
  (`LandingNodeHover`). The node of a branch once said "Safe Push failed on IJAI-4184 as it is: fix it or restart it"
  in a tooltip, nothing of it clickable. A text inside a button (the toggle of an island header) is no place for a
  link: the button that opens its object stands beside it.
- **An action runs once**: while it runs ([Running Operations](../design/operations.md)), its item is running on every
  surface (disabled, a spinner in a menu), a second start joins it, and the actions that must wait for it (every other
  rewrite of the repository) are disabled with its busy state as the reason: "Unstacking tool-events…". The spinner is
  where it was started: the button that stays (the notice's "Unstack from air", a form, a dialog), else the "⋯" of the
  object's row. Three clicks on an Unstack that nothing disabled once ran three unstacks, two of them failing with
  "tool-events is not stacked".
- **Disabled with a reason, not hidden**: an action that does not apply now stays in the list, disabled, and says why
  on hover (`description`); it disappears only where it never applies (no pull request, no robot).
- **A step the app can do is part of the action**: a push an action needs, a wait for the sync, a fetch run inside the
  action, which keeps its plain name ("Create Pull Request…", "Safe Push", "Restart Safe Push", "Request
  Review…"); its progress shows in the busy button ("Pushing 45%"). Never disable an action with "push first" or "sync
  and try again", and never narrate the step in a note. Only a consequence a person decides is said, and asked: a push
  that drops someone's remote commits, one that dismisses approvals, a start that cancels a running robot. The row of
  an unpushed branch once offered a disabled "Create Pull Request…" ("Push first: a pull request needs the remote
  branch") beside its own push button.
- **An unknown step is no action**: a wait for a fact that decides which action it is (the pull requests GitHub
  hasn't listed, the approvals Patronus hasn't checked) is no step of an action, as a push is: the action is not known
  yet. The slot says "Checking…" muted with its spinner, disabled, what it waits for on hover ("Waits for GitHub to
  list the pull requests"); every other surface of the action is disabled with the same words, and S says them. Never
  the action the facts would decide without the answer: a "Safe Push" that turned into "Awaiting Review" three
  seconds later had let a click start a robot whose approvals didn't count
  ([Next Step](../../spec/branch/next-step.spec.md#facts-without-an-answer)). A failed answer is said as such, never as
  "none".
- **A step is not a button beside its action**: a surface that offers the action doesn't offer its step too. The row,
  the preview and the review window of a branch the remote lacks show Create Pull Request… and Safe Push, which push
  it, and no "Push" of their own; the menu keeps the push, so it stays reachable. One name per action: the first push
  is "Push (3)" like every other, its description says that it creates the remote branch.
- **Never offer an action whose failure is known**: disable it with the reason; a wait is a quiet step that opens its
  object. "Safe Push" of a branch whose code owner is still asked is disabled with "Waits for the approval of IJPL
  Code Platform: Ilya Korennoy is requested on #1078", and the row shows the wait, not the doomed action
  ([Next Step](../../spec/branch/next-step.spec.md)). One model of an object decides its next step
  (`getBranchNextStep`), every surface renders it.
- **A state as a label**: the one exception to "a button is a verb". A quiet step names the state it waits in ("Awaiting
  Ilya Korennoy", "Changes Requested", "Draft"), muted (`text-muted-foreground`, `destructive` for changes requested), in
  Title Case as a button; its hover says the whole state and the action it takes ("Open Pull Request #1078" with its
  key), and the chip of the object keeps its own state, so the row doesn't say it twice.
- **The review keys of every object**: D reviews it in its window ("Review Changes": the commit, the branch, the pull
  request; one action, no "Open in Window"), from the row, its menu and its preview (D, the footer button, Enter on a
  file of the tree, which closes the preview); a commit of a branch is reviewed in the window of its branch, at the
  commit, where the stack and the actions are; Space previews it, Escape closes the window. On a row D follows the
  pointer as Space does; the keys that act (P, S, O, M) need the focused row. Inside a review J/K are the files, N/P
  the changes, ⌥↑/⌥↓ the commits.
- **No verb on a verb**: an action named by a verb is its own button ("Safe Push", never "Start Safe Push"; "Dry
  Run", never "Run Dry Run"); the toast says what happened ("Safe Push started").
- **One name per action**: "Open Pull Request" (never "Open on GitHub"), "Copy Pull Request Link", "Open in Patronus",
  "{Action} {mode}" for a robot ("Restart Safe Push", also when it pushes first; "Cancel Dry Run"; a bare "Cancel"
  reads like closing the popover), "Safe Push" for the start. Advice in a sentence names the action as its button
  does ("Restart Safe Push to test the current branch.").
- **A column of steps says it short**: the Safe Push slot of a row and of an island header and the landing overview
  show a step in a column as wide as its widest step, so a step there has a `compactLabel` where its name holds a
  name or a target the place or the hover already says: "Retarget #7" (the menu "Retarget #7 to master"), "Awaiting
  Review" ("Awaiting Ilya Korennoy" in the hover, the notice and the menus), "Create Pull Request…" in the header of
  the whole stack ("Create Pull Request of the Stack…" elsewhere). It starts with the words of the name, so the name
  stays one. A step that runs keeps the width it had at rest: its push the spinner and "45%" as the compact push of the
  row, "Pushing 45%" and the branches it pushes in its name and hover; a robot's time "47m · ~58m", "about 58 min
  left" in the hover. One name
  of a person in a column once made every card of the view 330px wide in its Actions.

## How to

- A toast of an event of an object (a robot, a push, a merge, a review, an issue): `showEventToast({ key, render })`
  from `src/lib/event-toast.ts`, with its key from `toastKeys`, `supersedes` for the events it takes the place of,
  `aliases` for the keys a later event finds it by, and `watch` for the stores its `render` reads;
  `replaceEventToast` for a state that is no news of its own. A robot's: `showRobotEventToast`
  (`src/lib/robot-toast.ts`); a merge's: `showMergeToast` (`src/lib/landing-toast.ts`).
- A success or info toast of nothing that follows: `showToast({ title, description, action })` from
  `src/lib/toast-message.ts` (`type: "info"` for a precondition), or `toast.add` from `@/components/ui/toast` for a
  custom one (a loading toast, a custom body).
- A history rewrite with an Undo: `showUndoToast(title, undo)` from `src/lib/command.ts`.
- A rewrite (a regroup, a squash, a reword, an undo): `runRewrite(spec, failureTitle, command, applied)` from
  `src/lib/rewrite.ts`, its toast with Undo after the sync.
- An inline form's action: `withRowProcessing(key, operation, fn, { success, errorTitle })` or `withRowProgress` when
  the action is an operation that toasts itself (`src/stores/inline-row-action.ts`).
- A toast that Activity records takes its words from the function the entry uses: `describePush`
  (`src/lib/push-summary.ts`), `describeRobot` (`src/lib/patronus.ts`, through `announceRobot` in
  `src/lib/patronus-actions.ts`), `appliedUpdateToast` (`src/lib/landed.ts`).

## References

- [UI Theme](../design/ui-theme.md): colours, states, the copy button.
- [Activity](../../spec/repository/activity.spec.md): the log in the words of the toasts.
- [Branch row](../../spec/branch/branch-row.spec.md), [Patronus](../../spec/branch/patronus.spec.md): the branch menu, the
  row keys and the robot actions built this way.
