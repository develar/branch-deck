---
name: Review Concerns
description: Requirements for the concerns of a review (experimental), the tasks a range does, each a set of parts of its units joined by evidence the plan read already (a changed declaration and the changes that name it, the files of a group, a rare identifier the changed lines of several files share), the rest attached to the concern of its file or its subject, a generated file to the concern of the source its header names, what no evidence joined last; in the order of the story, with an id a growing concern keeps and a name by its evidence.
style: plain-1
targets:
  - crates/review-plan/src/concerns.rs
  - crates/review-plan/src/units.rs
  - crates/bd-rewrite/src/review_plan_command.rs
  - src/lib/file-tree.ts
  - src/presenters/review-concerns.ts
  - src/presenters/review-units.ts
  - src/components/commit/CommitReview.tsx
  - src/components/files/FileTree.tsx
---

# Review Concerns

Status: Experimental
Date: 2026-10-31

## Purpose

The units cut a review into the changes a reviewer reads once, but a branch does several tasks, and the files of one
task lie all over the tree: a server dropped from six configs in five directories, a changed function and the files
that call it. A commit is no task either: an agent's round mixes tasks, and five commits change one guide.

A concern is one task of the range: a set of parts of its units, joined only by evidence the plan read already, no
git process and no accio, so the plan before the usages and the plan with them group alike. The window lists the
concerns, each with its files, and the story of the diffs follows them. The vision, the research behind the edges and
the measurements: [Change Review](../../../../docs/design/change-review.md).

## Goals

- Join only on evidence, with near no false join: a wrong join hides one task inside another.
- Join across languages and formats: a config, a guide and a test of one task.
- Cost no git process and a few milliseconds per range.
- Keep the id of a concern while it grows, so that a later review knows it.

## Non-goals

- A name by a language model: a concern is named by its evidence.
- Two tasks inside one block of a file of no language, or inside one declaration both changed: such a block is one
  node, and joins both.
- Moves between files: the units see a removal and an addition.

## Requirements

### Requirement: Nodes: A node is a unit, a block of a file of no language, or a file

A node MUST be a unit; one per block for a unit `other` of a file of no language the summaries parse; one per file
for a repeated edit and for `deletedFiles`.

#### Scenario: The configs and the deleted config of one dropped server
- **WHEN** a range drops a server from configs of no language and deletes another config
- **THEN** each block of the changed configs is a node, and the deleted config is one node
- **AND** they join into one concern by the identifier they share
  [@test] crates/review-plan/src/concerns_test.rs (`the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)

#### Scenario: A repeated edit
- **WHEN** a unit is a repeated edit
- **THEN** it gives one node per file

### Requirement: Nodes: A mechanical unit gives no node

A mechanical unit MUST give no node. A generated file, a build file and a file of imports alone SHALL be allowed to
follow a concern as a consequence of it ([The Rest](#the-rest)).

#### Scenario: A mechanical unit
- **WHEN** a unit is mechanical
- **THEN** it gives no node
- **AND** a generated file, a build file or a file of imports alone among it may follow a concern as its consequence

### Requirement: Edges: A changed declaration joins the changes that name it

A node that changes the form of a declaration, adds or removes one (`shape`, `added`, `removed`; a change of a body
alone forces no use to change) and a node of another unit whose changed lines name it as a whole word MUST join, when
the second is in the same file or in a file that links to the first by that name (`links`).

#### Scenario: A changed function and the change of its call in a file that imports it
- **GIVEN** a function whose form changes, a file that imports it and changes its call, and a change elsewhere
- **WHEN** the plan groups the range
- **THEN** the function and the change of its call are one concern
- **AND** the change elsewhere is not in it
  [@test] crates/review-plan/src/concerns_test.rs (`a_changed_declaration_and_the_changes_of_its_uses_are_one_concern`)

### Requirement: Edges: A short name joins nothing

A name of fewer than 3 characters MUST join nothing.

#### Scenario: A name of two characters
- **WHEN** a changed declaration has a name of fewer than 3 characters
- **THEN** the changes that name it do not join it by that name

### Requirement: Edges: A declaration that many nodes name joins none of them

A declaration that more than 6 nodes name MUST join none of them: a hub is no evidence.

#### Scenario: A declaration that seven changes name
- **WHEN** a changed declaration is named by the changed lines of more than 6 nodes
- **THEN** it joins none of them
  [@test] crates/review-plan/src/concerns_test.rs (`a_declaration_named_by_many_changes_joins_none`)

### Requirement: Edges: The nodes of a group join

The nodes of a group MUST join.

#### Scenario: A group of files
- **WHEN** nodes belong to one group
- **THEN** they join

### Requirement: Edges: A rare identifier joins nodes of two files

A rare identifier MUST join nodes of two files: a run of letters, digits, `_` and `-` that starts with a letter, and
its parts between `_` and `-`, lowercase, of 4 or more characters, no keyword, common word or common key of a config,
no part of a changed path (`-` and `_` alike); a token a node both removes and adds is no change of it.

#### Scenario: Configs that drop one server
- **WHEN** configs of two or more files remove the same identifier
- **THEN** that identifier joins their nodes
  [@test] crates/review-plan/src/concerns_test.rs (`the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)

#### Scenario: A token removed and added by one node
- **WHEN** a node both removes and adds a token
- **THEN** the token is no change of that node

### Requirement: Edges: A rare token is in few nodes of two or more files

A rare token MUST be all removed, or all added, in 2 to 10 nodes of two or more files, and in a range of 16 nodes or
more in a quarter of them at most.

#### Scenario: A token of a large range
- **GIVEN** a range of 16 nodes or more
- **WHEN** a token is in more than a quarter of its nodes
- **THEN** it is no rare token

#### Scenario: A token in 2 to 10 nodes
- **WHEN** a token is all removed, or all added, in 2 to 10 nodes of two or more files
- **THEN** it is a rare token

### Requirement: Edges: Two nodes join when their rare tokens are alike

Two nodes MUST join when their rare tokens are alike: the cosine of their weights (`ln((n + 1) / df)` over the nodes)
at 0.35 or more, or, with 4 or more shared tokens, 80% of the weight of the smaller in the larger. The pairs most alike
MUST join first.

#### Scenario: The cosine of the rare tokens
- **WHEN** the cosine of the weights of the rare tokens of two nodes is 0.35 or more
- **THEN** the two nodes join

#### Scenario: Four or more shared tokens
- **WHEN** two nodes share 4 or more rare tokens and 80% of the weight of the smaller is in the larger
- **THEN** the two nodes join

#### Scenario: The order of the joins
- **WHEN** several pairs of nodes are alike
- **THEN** the pairs most alike join first
  [@test] crates/review-plan/src/concerns_test.rs (`one_shared_identifier_bridges_no_two_concerns`)

### Requirement: Edges: A token is a name of code, never a word of prose

A token MUST be a name of code, never a word of prose: of a Markdown or text file only the code it quotes (a backtick
span, and of Markdown a fence and an indented line).

#### Scenario: A word of prose and the same kind of word quoted as code
- **WHEN** two Markdown files add the same word in prose, and two others the same word in a backtick span
- **THEN** the word of prose joins nothing
- **AND** the word quoted as code joins its two files
  [@test] crates/review-plan/src/concerns_test.rs (`a_word_of_prose_or_of_a_comment_joins_nothing`)

#### Scenario: An indented line of a text file
- **WHEN** two text files add the same words on an indented line
- **THEN** they join nothing: an indented line is code only of Markdown
  [@test] crates/review-plan/src/concerns_test.rs (`a_sentence_a_config_quotes_joins_by_its_compound_names_alone`)

### Requirement: Edges: The tokens of a source file

Of a source file a token MUST come from no comment line, and MUST be only a name a changed file declares on either
side or a word of a string literal (quoted by `"`, in TypeScript and JavaScript also by `'` and `` ` ``), never a name
of a library (`list`, `as_bytes`).

#### Scenario: A word of a comment
- **WHEN** two source files add the same word in a comment line
- **THEN** it joins nothing
  [@test] crates/review-plan/src/concerns_test.rs (`a_word_of_prose_or_of_a_comment_joins_nothing`)

#### Scenario: A name of a library and a word of a string literal
- **WHEN** two files both start calling the same name of a library, and two others log the same word of a string literal
- **THEN** the name of a library joins nothing
- **AND** the word of the string literal joins its two files, in TypeScript also quoted by `'` and `` ` ``
  [@test] crates/review-plan/src/concerns_test.rs (`a_name_of_a_library_joins_nothing_a_word_of_a_string_does`)

### Requirement: Edges: The tokens of a config

Of a config a token MUST come from every line but the key of a field, a key whose own value follows on the line and is
no object (`"side": "ide"`, `"answers": []`, `timeout = 30`), which every entry of a schema repeats; the key of an
object names an entry (`"ijproxy": {`, `[mcp_servers.ijproxy]`).

#### Scenario: The key of a field every entry of a schema repeats
- **WHEN** two configs add entries whose fields have the same keys (`"answers": []`)
- **THEN** those keys join nothing
  [@test] crates/review-plan/src/concerns_test.rs (`a_sentence_a_config_quotes_joins_by_its_compound_names_alone`)

#### Scenario: The key of an object
- **WHEN** configs change the entry the key of an object names (`"ijproxy": {`, `[mcp_servers.ijproxy]`)
- **THEN** that key is a token

### Requirement: Edges: A sentence a config or a literal quotes joins by its compound names

Of a sentence a config or a literal quotes (a string with a space) only its compound names MUST count, with `_`, `-`,
a digit or a capital past the first letter (`sessions-window-show`, `toolWindowId`).

#### Scenario: Sentences of configs, text files and string literals
- **WHEN** configs, text files and string literals add sentences that share words, and two configs a sentence with the
  same compound name
- **THEN** the shared words join nothing
- **AND** the compound name joins its two configs
  [@test] crates/review-plan/src/concerns_test.rs (`a_sentence_a_config_quotes_joins_by_its_compound_names_alone`)

### Requirement: Edges: One shared token bridges no two concerns

A pair of tokens MUST join two concerns of more than one node each only when two rare tokens have nodes in both: one
shared word bridges no two concerns.

#### Scenario: Two concerns that share one identifier
- **GIVEN** two concerns of two nodes each, joined by their own rare tokens
- **WHEN** one rare token has nodes in both
- **THEN** the two concerns stay apart
  [@test] crates/review-plan/src/concerns_test.rs (`one_shared_identifier_bridges_no_two_concerns`)

### Requirement: The Rest: The rest is off unless `--concern-edges` names it

The rest MUST be off unless `--concern-edges` names it: measured, it adds recall and costs more purity than it is worth
([Measurements](#measurements)).

#### Scenario: The edges by default
- **WHEN** `--concern-edges` does not name the rest
- **THEN** a node joined to nothing joins no concern by its file or its subject

### Requirement: The Rest: With it on, a lone node joins the concern of its file or subject

With the rest on, a node joined to nothing MUST join the concern of its file with the most changed lines, else a test
the concern of its subject (a file of the stem it names).

#### Scenario: A test and a change alone in a file
- **GIVEN** the rest is on
- **WHEN** a test has no evidence of its own, and a change alone in a file has none either
- **THEN** the test joins the concern of its subject
- **AND** the change joins the concern of its file
  [@test] crates/review-plan/src/concerns_test.rs (`a_test_follows_its_subject_and_a_change_alone_the_concern_of_its_file`)

### Requirement: The Rest: With it on, a generated file follows the file its header names

With the rest on, a generated file MUST follow the concern of the changed file its header names by its whole path, of
the most changed lines, at the end of its parts; a glob (`edit docs/*`) names none, and such a file stays in the
Mechanical group.

#### Scenario: A header that names its source by its whole path
- **GIVEN** the rest is on
- **WHEN** the header of a generated file names a changed file by its whole path
- **THEN** the generated file follows the concern of that file, of the most changed lines, at the end of its parts

#### Scenario: A header that names a glob
- **WHEN** the header of a generated file names a glob (`edit docs/*`)
- **THEN** it names no file, and the generated file stays in the Mechanical group

### Requirement: The Rest: A consequence follows the concern of its cause

A consequence MUST follow the concern of its cause, folded at the end of its parts, also with the rest off; of
several, the one of the most changed lines among the causes.

#### Scenario: A consequence with the rest off
- **GIVEN** the rest is off
- **WHEN** a consequence has a cause in a concern
- **THEN** it follows that concern, folded at the end of its parts
  [@test] crates/review-plan/src/concerns_test.rs (`a_generated_file_follows_the_concern_of_the_source_its_header_names`)

#### Scenario: A consequence of several causes
- **WHEN** a consequence has several causes
- **THEN** it follows the concern of the cause of the most changed lines

### Requirement: The Rest: A generated file is a consequence of the file its header names

A generated file MUST be a consequence of the changed file its header names by its whole path.

#### Scenario: A generated guide
- **WHEN** a generated file's header names a changed guide by its whole path
- **THEN** the generated file follows the concern of the guide
  [@test] crates/review-plan/src/concerns_test.rs (`a_generated_file_follows_the_concern_of_the_source_its_header_names`)

### Requirement: The Rest: A mechanical build file follows the code of its module that needs it

A build file whose change is mechanical (its dependency entries alone, or of the build model by its name) MUST be a
consequence of the code of its module (a file below its directory) that a link the commits add between two modules, or
one they remove, starts from, the code that needs the new dependency or no longer needs the old one.

#### Scenario: A new dependency of a module
- **WHEN** the commits add a link from the code of a module to another module, and the build file of the module adds
  the dependency
- **THEN** the build file follows the concern of that code
  [@test] crates/review-plan/src/concerns_test.rs (`a_build_file_follows_the_code_of_its_module_that_needs_another_module`)

#### Scenario: A removed dependency of a module
- **WHEN** the commits remove a link from the code of a module to another module, and the build file drops the
  dependency
- **THEN** the build file follows the concern of the code that no longer needs it

### Requirement: The Rest: A version alone in a build file joins nothing

A change of a build file that is a version alone MUST join nothing and stay in the Mechanical group.

#### Scenario: A version bump
- **WHEN** a build file changes a version alone
- **THEN** it joins nothing and stays in the Mechanical group

### Requirement: The Rest: A file of imports alone follows the declaration it imports

A file whose imports alone changed MUST be a consequence of a new, removed or changed declaration its import lines
name, of a file it links to.

#### Scenario: An import of a new declaration
- **WHEN** a file changes only its imports, which name a new declaration of a file it links to
- **THEN** it follows the concern of that declaration
  [@test] crates/review-plan/src/concerns_test.rs (`a_file_of_imports_alone_follows_the_declaration_it_imports`)

### Requirement: The Rest: What no edge joined is the last concern

The nodes no edge joined MUST be the last concern, `rest`.

#### Scenario: Nodes no edge joined
- **WHEN** some nodes are joined by no edge
- **THEN** they are the last concern, `rest`

### Requirement: Order, Identity, Name: The concerns come in the order of the story

The concerns MUST come in the order of their first unit in the story, `rest` last.

#### Scenario: Concerns and the rest
- **WHEN** the plan lists the concerns of a range
- **THEN** each comes at the place of its first unit in the story
- **AND** `rest` comes last

### Requirement: Order, Identity, Name: The parts of a concern come in the order of the units

The parts of a concern MUST come in the order of the units, one per unit and file, with the blocks it holds of a file
of no language (`null` for all).

#### Scenario: A concern of several parts
- **WHEN** a concern holds parts of several units and files
- **THEN** its parts come in the order of the units, one per unit and file
- **AND** each names the blocks it holds of a file of no language, `null` for all

### Requirement: Order, Identity, Name: A file of several concerns has its main row in one

A file of several concerns MUST have its main row in the one whose parts of it change the most lines, the first of
equal ones; each other one lists it in `elsewhere`.

#### Scenario: A file of two concerns
- **WHEN** a file holds parts of two concerns
- **THEN** its main row is in the concern whose parts of it change the most lines
- **AND** the other concern lists it in `elsewhere`
  [@test] crates/review-plan/src/concerns_test.rs (`a_file_of_two_concerns_has_its_main_row_in_the_one_of_its_most_changed_lines`)

#### Scenario: Equal changed lines
- **WHEN** the parts of a file in two concerns change as many lines
- **THEN** its main row is in the first of them

### Requirement: Order, Identity, Name: A generated file is in one concern only

A generated file MUST be in one concern only.

#### Scenario: A generated file
- **WHEN** a generated file follows a concern
- **THEN** no other concern holds it

### Requirement: Order, Identity, Name: A concern keeps its id as it grows

`id` MUST be the fingerprint of the first part (its unit's fingerprint, its file, its block), so that a concern that
grows keeps it; `fingerprint` MUST be the digest of the set of the fingerprints of its parts.

#### Scenario: A concern that grows
- **WHEN** a later range adds a part to a concern
- **THEN** its `id` stays the same
- **AND** its `fingerprint` changes
  [@test] crates/review-plan/src/concerns_test.rs (`a_concern_keeps_its_id_as_it_grows`)

### Requirement: Order, Identity, Name: A concern a token joined is named by its token

The name MUST be the evidence of the concern: of a concern a token joined, the rare token with the most of its nodes in
it (more than half), a whole identifier before a part of one (`zebrafish-mode`, not its `zebrafish`), a simple word
before a compound one, the one its changed lines name first.

#### Scenario: Configs joined by an identifier
- **WHEN** a token joined the configs that drop one server
- **THEN** the concern is named by that token
  [@test] crates/review-plan/src/concerns_test.rs (`the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share`)

#### Scenario: A whole identifier before a part of one
- **WHEN** the changed lines of a concern name a whole identifier and a part of it (`zebrafish-mode`, `zebrafish`)
- **THEN** the concern is named by the whole identifier
  [@test] crates/review-plan/src/concerns_test.rs (`a_name_of_a_library_joins_nothing_a_word_of_a_string_does`)

### Requirement: Order, Identity, Name: A removed and an added token of the same nodes are a replacement

A removed and an added token of the same nodes MUST name the concern `replacement`.

#### Scenario: One word replaced by another
- **WHEN** the nodes of a concern remove one token and add another
- **THEN** the concern is named `replacement` of the two
  [@test] crates/review-plan/src/concerns_test.rs (`a_generated_file_follows_the_concern_of_the_source_its_header_names`)

### Requirement: Order, Identity, Name: Without a token, a declaration, a file or a directory names it

A concern no token names MUST be named by its first changed declaration; else by its one file (`file`: a source with
the generated copies that follow it); else by the directory its files share.

#### Scenario: A changed declaration
- **WHEN** no token names the concern and it changes a declaration
- **THEN** the concern is named by its first changed declaration
  [@test] crates/review-plan/src/concerns_test.rs (`a_changed_declaration_and_the_changes_of_its_uses_are_one_concern`)

#### Scenario: A source and its generated copies
- **WHEN** the concern is one source with the generated copies that follow it, and no other evidence
- **THEN** the concern is named by its one file, `file`
  [@test] crates/review-plan/src/concerns_test.rs (`a_source_and_its_generated_copies_are_named_by_the_file`)

#### Scenario: Files of one directory
- **WHEN** no token, declaration or one file names the concern
- **THEN** it is named by the directory its files share

### Requirement: Order, Identity, Name: A token is named as the changed lines write it

A token MUST be matched in lowercase and named, in the name and in its evidence, as the changed lines of the concern
write it most often (`activatesProjectViewOnStartup`).

#### Scenario: A compound name written with capitals
- **WHEN** a concern is joined by a token its changed lines write with capitals
- **THEN** the name and its evidence write the token as the changed lines do most often
  [@test] crates/review-plan/src/concerns_test.rs (`a_sentence_a_config_quotes_joins_by_its_compound_names_alone`)

### Requirement: Order, Identity, Name: A backticked Kotlin name is shown without its backticks

A declaration name wrapped in Kotlin backticks MUST be shown without one surrounding pair; the unit's source name is
unchanged.

#### Scenario: A backticked Kotlin function
- **WHEN** a concern is named by a Kotlin function whose name is wrapped in backticks
- **THEN** the name is shown without one surrounding pair
- **AND** the unit's source name is unchanged
  [@test] crates/review-plan/src/concerns_test.rs (`a_concern_name_from_a_backticked_kotlin_function_is_unquoted`)

### Requirement: In `brd`: `brd review-plan` prints the concerns

`brd review-plan` MUST print `concerns` (`"schema": 29`): per concern its `id`, `name`, its `parts` (`unit`, `path`,
`blocks`), its `paths`, its `elsewhere` (since 8), its `evidence` (`kind`: `uses`, `group`, `token`, `rest`, its
`detail` and `joins`, the most joins first) and its `fingerprint`.

#### Scenario: The concerns of a range
- **WHEN** `brd review-plan` prints a range
- **THEN** it prints `concerns`, each with its `id`, `name`, `parts` (`unit`, `path`, `blocks`), `paths`, `elsewhere`,
  `evidence` (`kind`, `detail`, `joins`, the most joins first) and `fingerprint`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

#### Scenario: The sources of a name
- **WHEN** `brd review-plan` prints the `name` of a concern
- **THEN** it is `source` `token` with `token`, `replacement` with `removed` and `added`, `declaration` with `name`,
  `file` with `path`, `directory` with `directory`, or `rest`

### Requirement: In `brd`: The hidden `--concern-edges` picks the edges

The hidden `--concern-edges uses,tokens,rest` (or `none`) MUST pick the edges for a measurement of each; without it
`uses,tokens`.

#### Scenario: Without the flag
- **WHEN** `brd review-plan` runs without `--concern-edges`
- **THEN** the edges are `uses,tokens`

#### Scenario: Edges for a measurement
- **WHEN** `brd review-plan` runs with `--concern-edges` naming some of `uses,tokens,rest`, or `none`
- **THEN** only those edges join

### Requirement: In `brd`: The concerns run no git process

The concerns MUST run no git process.

#### Scenario: A review plan of a range
- **WHEN** `brd review-plan` plans a range
- **THEN** its concerns add no git process
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### Requirement: In the Review Window: Where the steps are the concerns, each step is its concern

The window lists the steps of the story ([Review Tour](../review-tour/spec.md#in-the-review-window)); where the steps
are the concerns (a range of one commit, or of commits without a body), each step MUST be its concern, as below.

#### Scenario: A range of one commit
- **WHEN** the window reviews a range of one commit, or of commits without a body, with its plan
- **THEN** each step of the story is a concern of the range
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: The tree lists the concerns at its top

With a concern beside the rest, the tree of a review with its plan MUST list the concerns at its top, in their order:
a row per concern, named by its evidence ("proxy", "grep → accio", the name of a declaration, the directory or
"Root"), with its files and lines, why its parts are together in its hover
("Joined by the identifiers ij-mcp-proxy, ij-proxy").

#### Scenario: The rows of the concerns
- **GIVEN** a plan with concerns beside the rest
- **WHEN** the tree of the review shows
- **THEN** a row per concern stands at its top, in their order, named by its evidence, with its files and lines
- **AND** its hover says why its parts are together
  [@test] src/lib/file-tree.test.ts (`lists the concerns, each with its files by their path, a file of another concern also, its generated files folded; then Other changes and Mechanical`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

#### Scenario: The name and the hover of a concern
- **WHEN** a concern is named by a token, a replacement, a declaration, a file or a directory
- **THEN** its row names it by that evidence ("proxy", "grep → accio", the name of a declaration, the directory or
  "Root")
- **AND** its hover says why its parts are together ("Joined by the identifiers ij-mcp-proxy, ij-proxy")
  [@test] src/presenters/review-concerns.test.ts (`names a concern by its evidence, and says why its parts are together`)

### Requirement: In the Review Window: A concern row holds its files and its generated files

Under the row of a concern the tree MUST list the files whose main row it holds, each named by its path, in the order
of its parts, then its generated files in a group per kind ("Generated (1)"), collapsed.

#### Scenario: The files of a concern
- **WHEN** a concern holds files and a generated file
- **THEN** its files are listed under it by their path, in the order of its parts
- **AND** its generated files follow in a group per kind ("Generated (1)"), collapsed
  [@test] src/lib/file-tree.test.ts (`lists the concerns, each with its files by their path, a file of another concern also, its generated files folded; then Other changes and Mechanical`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: Other changes and the Mechanical group follow the concerns

After the concerns the tree MUST list the files of `rest` and of no concern as the directory tree under
"Other changes", then the Mechanical group of the other mechanical files.

#### Scenario: The files no concern holds
- **WHEN** the tree lists the concerns of a plan
- **THEN** the files of `rest` and of no concern follow as the directory tree under "Other changes"
- **AND** the Mechanical group of the other mechanical files comes last
  [@test] src/lib/file-tree.test.ts (`lists the concerns, each with its files by their path, a file of another concern also, its generated files folded; then Other changes and Mechanical`)

### Requirement: In the Review Window: Without concerns the tree is the one of the review plan

Without the plan, or with no concern but `rest`, the tree MUST be the one of
[Review Plan](../review-plan/spec.md#in-the-review-window).

#### Scenario: A plan of no concern but the rest
- **WHEN** the plan has no concern but `rest`, or there is no plan
- **THEN** the tree is the one of the review plan, without concerns
  [@test] src/lib/file-tree.test.ts (`is the tree without concerns while the plan has none but the rest, and leaves out a concern the filter empties`)

### Requirement: In the Review Window: A file of several concerns has a row "also in" in each other

A file MUST have one row of its own, in the concern of its main row; each other concern of it (`elsewhere`) a row of
its own name, "docs/guide.md, also in 1 · ijproxy" ("in step 2" after its name; a concern of a story that tells no
step by its name), in its place among the files (in the directory tree of Other changes for `rest`), its main concern
in its hover.

#### Scenario: A file of two concerns
- **WHEN** a file is in two concerns
- **THEN** it has one row of its own in the concern of its main row
- **AND** the other concern lists it in its place among its files as a row of its own name,
  "docs/guide.md, also in 1 · ijproxy", its main concern in its hover
  [@test] src/lib/file-tree.test.ts (`lists the concerns, each with its files by their path, a file of another concern also, its generated files folded; then Other changes and Mechanical`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

#### Scenario: A story that tells no step by its name
- **WHEN** the concern of the main row is of a story that tells no step by its name
- **THEN** the row says "in step 2" after its name

#### Scenario: A file of the rest that is also in a concern
- **WHEN** a file of `rest` is also in a concern
- **THEN** its row "also in" stands in its place in the directory tree of Other changes
  [@test] src/lib/file-tree.test.ts (`shows a file of what no evidence joined that is also in a concern in its place under Other changes`)

### Requirement: In the Review Window: A row "also in" shows the part of its concern

A row "also in" MUST select the file and show its part of that concern.

#### Scenario: Choosing a row "also in"
- **WHEN** the user chooses the row "also in" of a file in a concern
- **THEN** the file is selected and its part of that concern shows
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: A row "also in" counts as no file

A row "also in" MUST count as no file: "File 4 / 8" is the place of the file, and the line counts of a concern are
those of its own rows; the line counts of a directory under "Other changes" exclude also rows, and a directory with
only those rows has no line-count badge.

#### Scenario: The place of a file and the line counts of a concern
- **WHEN** a concern holds a row "also in"
- **THEN** "File 4 / 8" counts the place of the file, not the row
- **AND** the line counts of the concern are those of its own rows
  [@test] src/lib/file-tree.test.ts (`lists the concerns, each with its files by their path, a file of another concern also, its generated files folded; then Other changes and Mechanical`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

#### Scenario: A directory of Other changes with rows "also in"
- **WHEN** a directory under "Other changes" holds rows "also in"
- **THEN** its line counts exclude them
- **AND** a directory with only those rows has no line-count badge
  [@test] src/lib/file-tree.test.ts (`shows a file of what no evidence joined that is also in a concern in its place under Other changes`)

### Requirement: In the Review Window: The story follows the concerns

The story MUST follow the concerns (`presentNarrative`): the parts of each concern together, in its order, its first
section under its name on the divider of its step ("Step 1 · ijproxy", `review-concern-heading`), `rest` under
"Other changes".

#### Scenario: The story of two concerns and the rest
- **WHEN** the window tells the story of a plan with concerns
- **THEN** the parts of each concern stand together, in its order
- **AND** its first section is under its name on the divider of its step ("Step 1 · ijproxy", `review-concern-heading`)
- **AND** `rest` is under "Other changes"
  [@test] src/presenters/review-units.test.ts (`lays the parts of each concern out together under its name, a unit cut by the concerns in each, its generated file collapsed at its end, the mechanical groups after`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: A unit of several concerns shows in each with its blocks

A unit of several concerns MUST show in each with the blocks it holds there, also a file of no language the story shows
whole, the blocks of another concern one row "Shown in grep → accio".

#### Scenario: A guide of two concerns
- **WHEN** a unit holds blocks of two concerns
- **THEN** it shows in each concern with the blocks it holds there, also a file of no language the story shows whole
- **AND** the blocks of the other concern are one row "Shown in grep → accio"
  [@test] src/presenters/review-units.test.ts (`lays the parts of each concern out together under its name, a unit cut by the concerns in each, its generated file collapsed at its end, the mechanical groups after`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: Generated files and mechanical groups come at the end

A generated file of a concern MUST be collapsed at its end, saying why; the mechanical groups MUST come after the
concerns.

#### Scenario: A generated file of a concern and the mechanical groups
- **WHEN** a concern holds a generated file, and the range has mechanical groups
- **THEN** the generated file is collapsed at the end of its concern, saying why
- **AND** the mechanical groups come after the concerns
  [@test] src/presenters/review-units.test.ts (`lays the parts of each concern out together under its name, a unit cut by the concerns in each, its generated file collapsed at its end, the mechanical groups after`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: Without concerns the story keeps the order of the units

With no concern but `rest`, the story MUST keep the order of the units.

#### Scenario: A plan of no concern but the rest
- **WHEN** the plan has no concern but `rest`
- **THEN** the story keeps the order of the units
  [@test] src/presenters/review-units.test.ts (`keeps the order of the units while the plan has no concern but the rest`)

### Requirement: In the Review Window: A row and the section at the top select each other

A row MUST show the first open section of its file in its concern; the section at the top MUST select the row of its
file in its concern, the row "also in" there, as J and K step through the sections, also into the next concern.

#### Scenario: Choosing a row
- **WHEN** the user chooses a row of a file in a concern
- **THEN** the first open section of the file in that concern shows
  [@test] src/lib/file-tree.test.ts (`selects the row of a section in its concern, and shows the first open section of a row`)

#### Scenario: J and K through the sections
- **WHEN** J and K step through the sections, also into the next concern
- **THEN** the section at the top selects the row of its file in its concern, the row "also in" there
  [@test] src/lib/file-tree.test.ts (`selects the row of a section in its concern, and shows the first open section of a row`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

### Requirement: In the Review Window: The filter keeps the concerns of the files it shows

The filter of the files MUST keep the concerns of the files it shows.

#### Scenario: A filter that leaves files of two concerns
- **WHEN** the filter of the files shows files of two concerns
- **THEN** the tree keeps those concerns, each with the files it shows
  [@test] src/lib/file-tree.test.ts (`is the tree without concerns while the plan has none but the rest, and leaves out a concern the filter empties`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them`)

## User Experience

```json
"concerns": [
  {"id": "3f1c…", "name": {"source": "token", "token": "proxy"},
   "parts": [{"unit": "other:.codex/config.toml", "path": ".codex/config.toml", "blocks": null},
             {"unit": "deleted:dotnet/.codex/config.toml", "path": "dotnet/.codex/config.toml", "blocks": null}],
   "paths": [".codex/config.toml", "dotnet/.codex/config.toml"],
   "evidence": [{"kind": "token", "detail": "ij-mcp-proxy", "joins": 5}], "fingerprint": "9a02…"},
  {"id": "…", "name": {"source": "rest"}, "parts": […]}
]
```

The tree of the review window, the range of the `review_concerns` template (`review-plan-concerns-tree`):

```
◎ ijproxy                          +0 −10
    .codex/config.toml
    docs/guide.md  also
    tools/codex/config.toml
    .mcp.json
◎ grep → accio                     +3 −4
    docs/guide.md
    docs/tools.md
  › Generated (1)
≡ Other changes
  › src / App.kt
› Mechanical (1)
```

The first section of each concern opens with a divider on the canvas above its card: "Step 1 · ijproxy" in
`text-sm font-semibold` (`review-concern-heading`), then the heading of a group it starts, muted
(`review-section-heading`); the header of the file names the file alone.

## Open Questions / Risks

- A block of a file of no language that two tasks rewrote is one node and joins both; a guide that every commit of a
  range edits stays in `rest` or joins them.
- A declaration both tasks change is one unit and joins them: the plan cannot tell whose change it is.
- The stop words are a short list: a common word of prose that is no word of the list joins two blocks of prose when
  their cosine is high enough.
- The name of a token concern is a word, not a sentence: `proxy` for a server dropped from six configs.

## Measurements

The lab of 2026-10-31 (`~/projects/review-concerns-lab`, its `results/scores.md`): four corpora of tangled ranges
with a label per hunk, `brd review-plan` per range, each hunk mapped to the concern part whose blocks hold it; a part
of `rest` and a mechanical slice count as a group of its own. Purity is the share of groups of more than one hunk that
hold one task (Wilson 95% lower bound in parentheses); recall the share of the pairs of a task that share a group.
Merge precision alone says little: one group over a whole range scores 91–98% on it.

| Corpus | Cases, hunks | One group per file: recall / purity | `uses,tokens` (default): recall / purity | With `rest`: recall / purity |
|---|---|---|---|---|
| this repository, 2–3 atomic commits tangled | 200, 6047 | 72.9 / 91.4 (89.6) | 71.2 / 93.5 (91.6) | 74.7 / 90.8 (88.7) |
| tangled-ccs-commits (HF), 2–5 concerns | 245, 7750 | 80.8 / 100 (99.7) | 75.0 / 98.8 (98.0) | 78.3 / 98.7 (97.8) |
| jsoup, gson, commons-lang tangled | 200, 3511 | 81.4 / 89.9 (87.1) | 46.7 / 95.3 (93.2) | 57.0 / 88.4 (85.2) |
| idea-1 branches, a label per line by the commit | 94, 9451 | 77.8 / 84.7 (82.6) | 67.4 / 85.4 (83.2) | 79.3 / 81.0 (78.4) |

The other groupers on the same corpora, recall / purity: `inspect` (Ataraxy Labs, its entity groups; its entities
cover 14% of the idea-1 hunks, Kotlin barely, the rest one group per file) 62.8 / 90.9, 72.9 / 97.3, 58.1 / 88.4, 75.0
/ 84.9; `sem` (its entities) 33.7 / 96.8 on this repository, timing out on 5 idea-1 ranges; SmartCommit gave no group
on any range, so it has no score yet. RefactoringMiner, the oracle of moves and renames to come, takes 9.4 s median
on a Java range and 16 s (p95 101 s) on one of idea-1, where 17 of 30 ranges have a refactoring (75 moves and
renames): an oracle offline, never in a review.

The default is purer than a group per file on three corpora and joins changes across files and formats a group per
file never does; its recall is lower, the rest buys it back at the cost of purity, so it stays off. The rounds that
led here, each measured on the same corpora:

- The first token edge joined singletons by one shared word and chained a branch into one concern; a big block of
  prose shared words by chance. Fixed by the cosine of the rare tokens, the unchanged words of a block left out, and
  no token bridging two concerns.
- The `uses` edge from a change of a body alone joined callers that had nothing to do with it: purity of `uses` alone
  on idea-1 87.2 → 88.1, on Java 94.4 → 96.1 once only a changed form, a new or a removed declaration joins its uses.
- Words of prose and comments (`suite`, `same`, `compiles`) and names of libraries (`list`, `as_bytes`,
  `IllegalStateException`) joined tasks: tokens come from code only, of a source file only the names the change
  declares. Wrong token joins on idea-1 64 → 61, on the synthetic corpus 10 → 5; what remains are names the project
  declares that two tasks share (`turnEnded`).
- A crash of the symbol dependencies on `use m::m;` (5c20eed6) had scored 28 cases as singletons.
- A build file and a file of imports alone sat in the Mechanical group apart from the task that needed them: they
  follow its concern now, as a generated file its source. Of the mechanical slices that joined a concern, 230 of 232
  are right on this repository and 170 of 174 judged on idea-1; the false merges of mechanical slices fell from 27 to
  12 on idea-1, the recall rose (this repository 67.8 → 71.2), the purity of idea-1 eased 86.2 → 85.4.
- A branch of idea-1 that adds a step to a restart story showed the concerns "again" (joined by `does`), "side" and
  "params": the words of the sentences a JSON profile and a TypeScript string quote, the keys of the fields every entry
  of a schema repeats (`"side": "ide"`, `"params": {}`), and an indented line of a text file read as code. Of a
  sentence only its compound names count now, a key of a field none, and a token is named as it is written, a whole
  identifier before a part of one (v6, `results/bd-v6`). Recall / purity, v5 → v6: this repository 71.2 / 93.5 →
  70.6 / 93.1, tangled-ccs 75.0 / 98.8 → 74.6 / 99.2, Java 46.7 / 95.3 → 46.6 / 95.3, idea-1 by the commit of a line
  67.4 / 85.4 → 67.6 / 85.3. The purity of the groups eases where an impure group is cut in two (a concern of 135
  hunks of four tasks became two of three); the pairs of hunks it joins are truer: pair precision on idea-1 76.6 →
  77.8, by the label of the case 90.4 → 91.0. The joins it lost were right by chance: the words of the test titles of
  one commit.

A range takes 0.13–0.19 s median for the whole plan (concerns 12.7 ms for 168 units). The false merges left are of
three kinds: a concern (146 on idea-1), a mechanical slice that holds lines of two tasks (27), and a unit, one
declaration, that two tasks changed (43).
