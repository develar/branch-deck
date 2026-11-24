# Proposal

## Why

A shown review (show of ship/show/ask, #254) returned to the author with changes requested had no way back: the fix
landed by its own Safe Push and the review never saw it, so its reviewers looked at stale commits and its approval
could close it without the fix ever being reviewed. A draft whose robot fell out of the 7-day listing never opened, a
failed open or close was told once per session and never again, and `--after-landing --reviewer` patched a plan after
its fingerprint.

## What Changes

- A fix of the branch (`(branch) …` or its issue key) lands by its ordinary Safe Push; once its robot merged, the review
  takes it (`Intent::ExtendShownReview`; the app by the news `ShownReviewFixLanded`, `brd request-review <branch>`):
  the commits the robot landed replayed onto the head of the review (signed per `commit.gpgsign`) and pushed leased on
  the head the remote has, its reviewers asked again once GitHub shows it, the record of its body naming the fix last. A fix that
  doesn't apply leaves the head and is named in a comment, once. A repeat adds nothing.
- While a fix lands (its robot runs, or merged and not taken yet) the review is `extending`: neither the author's move
  nor approved, so nothing closes it.
- A draft whose branch the fetched baseline landed whole while no robot of it is listed opens on its head.
- A run of a news of a shown review that failed is told again at the next answer of the listing or the robots, three
  times per session at most.
- `brd request-review <branch>` with a shown review opens or extends it without a flag; `--reviewer` adds reviewers to
  it and its record, the plan rebuilt with them. `brd robot` names the review a Safe Push of a fix `extends`.

Capabilities: `branch/shown-reviews`, `cli/agent-cli`.
