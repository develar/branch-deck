# The Commit Body a Review Reads

**Updated:** 2026-11-06  
**Tags:** [commits, review, agents, writing]

The body of a commit is the one place where the author of a change, a person or an agent, tells its story at the
moment of writing it. A reviewer reads it before any diff: `git log` shows it, a pull request of Branch Deck is
written from it, an IDE shows it beside the change, and the review of Branch Deck tells each step by it
([Review Tour](../../spec/branch/review-tour.spec.md#the-story-of-a-commit)). This guide says how to write a body that
reads well in all of them. Nothing in it needs a tool: it is plain text with a few conventions of Markdown that read
as text too. Branch Deck reads a body by one grammar ([Commit Row → Message body](../../spec/commit/commit-row.spec.md)),
which `brd review-plan` prints with the story of each commit.

## In short

```text
the review window opens a step on the body of its commit          <- the subject: the behaviour change

A step of the review now opens on the body of its commit, above   <- the lead: the new state and why,
its first card, so the reviewer reads why before the diff.           1-3 sentences, under ~350 characters

The opening is measured where it is drawn (`useMeasuredHeights`):  <- a paragraph per decision,
its text wraps as the browser wraps it, so an estimate would          code names in backticks
overlap the card. `DiffLayout.cardTop` clamps a reveal to the card.

Look closely: a disclosure grows downward; a reader in the middle  <- where you ask the reviewer to decide
of an opening keeps the line under the pointer.

Tests:                                                             <- labels from the vocabulary,
- `layout.test.ts`: an opening of measured height, a growth           each right before its list
  downward, the prologue before the first card

Evidence:                                                          <- measurements indented
    100 steps: 41 ms to mount, 3 ms per re-measure

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>           <- trailers last
```

## The subject

The subject says what changes for the user of the code, as a sentence in the present tense: "the project open shows
the Project view only when it does not restore a stored layout", not "fix project view" or "refactor startup". It is
the title of the step everywhere: a tree row, the heading of the step, the line of `git log --oneline`. One sentence
of about 100 characters is fine; a second sentence belongs in the lead.

The subject has the form of the repository: its agent instructions say it, else its `git log` shows it (a ticket, a
label, a conventional type). A `(branch) ` prefix of Branch Deck goes first, and the form of the repository follows
it: `(dev-dist-cache) setup build: write the pinned Qodana date`. The push drops the prefix, so the rest alone must
pass the commit rules of the repository; `brd pick` refuses a message that they reject (`message_format`).

The diff shows what changed; the body says why. A body that only retells the diff gives a review nothing to judge.

## The lead

The first paragraph is the summary the contents of a review show for the step: the new state and why it is so, in 1
to 3 sentences, under about 350 characters. Name the state the change leaves, not only the old behaviour: of the
bodies we measured, 55% opened with the old behaviour and 4% named the new state in their first sentence, so the
contents of a review said what was wrong and never what is right now.

Keep the lead a paragraph of prose. A body that opens with a label or a heading has no lead (`lead: false`), and
nothing summarizes the step.

## A paragraph per decision

After the lead, give each decision its own paragraph: what you chose, what you did not and why, and name in backticks
the code it is about. The review of a commit alone takes such paragraphs as its steps, in your order, each with the code
it names and titled by its first sentence, so open it with a short sentence of what the step does (100 characters or
fewer); a list you write right under a paragraph is part of it, and `Tests:` naming the tests is the step of the tests.
A reviewer reads a long body by its blocks: Branch Deck marks where a body folds at a block boundary, after the lead
and the blocks that read in about 12 lines (`parts`, each `folded` or not), never between a label and its section; a
Look closely never folds, wherever it stands. A body cut into short blocks folds where a reader expects it; a body of
one long paragraph is all or nothing.

## Code names are anchors

Write a name of code in backticks, as the code spells it: `ToolWindowManagerImpl.isShownOnStartupAllowed`,
`text::Body::shown`, `Store#load`, `src/lib/diff/layout.ts`. Branch Deck resolves each of them to the declaration of
the change it names, with the line of its name, or to the file, each with its step, so a review can take the reader
from the name to that place in the diffs:

- a member by its owner (`Owner.member`; Rust `module::Type::member`; TypeScript `Type#member` or `Type.member`), a
  function or a type by its name alone when the change has only one of that name;
- a file by its path, a tail of its path, or its name when the change has only one file of that name; a line after it
  (`layout.ts:171`) is read past, the file is the target;
- a call with its parentheses (`open()`): a plain lowercase word in backticks (`changes`) is never a link, its
  parentheses make it a name;
- a name the change does not touch resolves to nothing and stays code in the text. `brd review-plan` lists every name
  in backticks with what it resolved to, `"target": null` where it resolved nothing: run it before you push to see
  which names do not bind.

A bare compound name (`toolWindowId`, `Owner.member`, `snake_case`) is resolved too, but only to a declaration or a
path, and only kept when it resolves: backticks say that you meant code, bare text says nothing, so a product name in
prose (`YouTrack`) never links to a file named for it. Do not put commands, flags or expressions in backticks
for a link; they read as code and link nowhere, which is right.

## Labels

A label heads the blocks after it, up to the next label: write it as a line of its own, `Label:`, right before its
list or paragraph, or at the start of a paragraph (`Why: the cache is per window`). Use the words of a small
vocabulary, so a reviewer finds the same section in every commit:

| Label | What follows |
|---|---|
| `Cause:` | why the old code was wrong, for a fix |
| `Fix:` | what the change does about it |
| `Tests:` | the tests that prove it, by file or name |
| `Evidence:` | what you ran and measured, on the final code |
| `Left out:` | what the change does not do or prove, and why |
| `Look closely:` | the decision you ask the reviewer to make |

"The cause", "Fix, in two places" and "Evidence, on the final code" are the same labels: a leading "The", the case and
what follows a comma do not matter. `Tests:` and `Evidence:` (and Gates, Verified, Verification, Results, Proof) are
the verification of the change (`kind: verification`): they cost no line of the fold and a review folds them wherever
they are, so a long account of what you ran never pushes the story out of sight. Put them after the story.
`Left out:` (`kind: leftOut`) is what a reviewer must know before approving: a case the change does not handle, a test
that can pass without proving what it names. A review shows it always, at no line of the fold, so write there only
what is true and only what the reader needs.

## Look closely

`Look closely:` is where you ask the reviewer to decide: a trade-off you made, a behaviour you were unsure of, a place
where a wrong guess costs the most. Branch Deck tells this label from the others (`kind: lookClosely`): it is the one
thing in a body that asks for an answer, so a review never folds it, wherever it is, and it costs no line of the fold.
Written at the start of a paragraph (`Look closely: the count is a constant for now…`), it heads that paragraph
alone, with a list or code you write right under it, no blank line between (the cases to check, one per item): what
you write after a blank line is the story again, so put the question in that one block. A request for a specific review is the strongest lever an author has on a
review that helps, so write one where it is true, and only there: a request in every commit is read as none.

## Measurements and code

Indent a measurement, a log or a snippet by four spaces, or put it in a fence (```` ``` ````): it keeps its lines and
its spacing everywhere, `git log` included, and Branch Deck keeps it a preformatted block, never joined into prose.
A table of Markdown reads as text in `git log` too, but keep it small.

## The order of a body

1. The lead.
2. A paragraph per decision, with its code names in backticks.
3. `Look closely:` where you ask for a decision.
4. The verification: `Tests:`, `Evidence:`, `Left out:`.
5. The trailers (`Co-Authored-By:`, `Signed-off-by:`), last, after a blank line. A body of trailers alone tells no
   story: the review treats its commit as a commit without a body.

## Why not `<details>` or git notes

Both would hide the verification from the story, and both fail outside one tool. `<details>` is HTML: GitHub folds
it, but `git log`, an IDE and every terminal show the tags as text. Git notes are not shown by `git log` by default,
are not pushed or fetched unless configured, and are lost on a rebase unless `notes.rewriteRef` is set; a commit of a
virtual branch is rewritten on every sync. The labels of the vocabulary mark the same sections for Branch Deck and read
as plain headings everywhere else.
