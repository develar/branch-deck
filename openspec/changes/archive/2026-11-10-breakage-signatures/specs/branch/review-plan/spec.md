## MODIFIED Requirements

### Requirement: Since Your Review: brd review-plan prints the interdiff and its schema version

`brd review-plan` MUST print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`, `commits`
with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, `message_change`, and `scope` with `base`,
`head` and `carried`), with `"schema": 31`, and its output MUST be in `brd schema`; the other outputs of `brd` keep
`"schema": 2`.

#### Scenario: The plan since the review
- **WHEN** `brd review-plan` runs on a marked branch
- **THEN** it prints the branch, `base`, `head` and the `interdiff` with `"schema": 31`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists the output of `brd review-plan`, and the output matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

#### Scenario: The versions of the schema
- **WHEN** a reader of an older plan reads the versions
- **THEN** version 3 has `files` in place of `risk` and `usages` in place of `fan_in`, version 4 the `units` ([Review Units](../review-units/spec.md)), version 5 the `generated` files with why, the lock files among them, version 6 a deletion as real work (no `mechanical: "deleted"`; the units `deletedFile` and `deletedFiles`), version 7 the `concerns` ([Review Concerns](../review-concerns/spec.md)), version 8 their `elsewhere`, version 9 the name `file`, version 10 the `steps` and the `brief` ([Review Tour](../review-tour/spec.md)), version 11 the `api` and `test` of a unit ([Review Units](../review-units/spec.md)) in place of its `public`, version 12 the `still_bound` and `by_name` usages and `unbroken` in place of `updated`, version 13 the `reached` and `unreached` tests of a step ([Review Tour](../review-tour/spec.md)) in place of `named` and `unnamed`, version 14 the story of each commit once (`steps.commits`, [Review Tour](../review-tour/spec.md#the-story-of-a-commit)) in place of the `body` of a commit of a step, and the `newApi` and `moduleLinks` of a step, version 15 the `parts` of a story where they are `told` in place of its `toldIn`, the `joined` units and the `marks` of a stop, the `shownElsewhere` units of a step, the state `reworded`, the `message_change` of a commit, the `change` of a story and the `removals` of its parts, version 16 the `paragraphs` of an entry of the brief, whole and changed by word, with their `links` and `excerpt`, in place of its `added` and `removed`, and its `blocks` in place of its `block` ([Review Tour](../review-tour/spec.md#brief)); version 19 added repeated replacements to the brief, version 20 a widening as an `added` entry with `widened`, the reach in the forms and the `verdict`, version 21 the form `row` of a paragraph of the brief, the `excerpt` of every changed paragraph of several sentences and no paragraph of front matter or metadata ([Review Tour](../review-tour/spec.md#brief)), version 22 the `table` block of a body ([Commit Row](../../commit/commit-row/spec.md#message-body)) and its `summary`, version 23 the `modules` of the changed files ([Modules](#modules)), the `module` of a file and the `moduleEdges` of a step in place of its `moduleLinks`, version 24 the `groups` of a large step ([Review Tour](../review-tour/spec.md#groups-of-a-step)) in place of the `concern` of a stop, version 25 the `registrations` ([Registrations](#registrations)), version 26 the `texts` of the UI ([UI Texts](#ui-texts)), version 27 the `folder` of each file ([Modules](#modules)), version 28 the `tested` of each group of a large step ([Groups of a Step](../review-tour/spec.md#groups-of-a-step)), version 29 the `entries` of a module the change adds ([Modules](#modules)), version 30 the `title` of a step, of a commit of a step and of a story as `{prefix, text}`, version 31 the forms `before` and `after` of the `api` with each parameter type as the source writes it and a Java parameter as Java writes it

### Requirement: API Surface: A type's constructors read as calls of it

A type's constructors MUST read as calls of it, a line each, each after the narrower of its reach and its type's, never
the name of a `Reach`; a parameter by its name and its type as the source writes it, `= …` for a default, Kotlin's
`vararg` before a vararg, Java's type before its name; a type of several names without one written (C#) MUST NOT be
shown, never as `List/Window`.

#### Scenario: The constructors and parameters of a type
- **WHEN** the form of a type with constructors is written
- **THEN** each constructor reads as a call, a line each: `Event(text: String, styles: List<StyleRange>, start: Long = …)`, `private Event()`
- **AND** each after the narrower of its reach and its type's (`internal Port(value: Int?)` of an internal class)
- **AND** a Java one as Java writes it: `Port(int value, String... names, int[][] grid)`
- **AND** a type of several names without one written is not shown, never as `List/Window`
  [@test] crates/review-plan/src/shown_test.rs (`a_form_of_constructors_and_parameters_reads_as_code`)

### Requirement: In the Review Window: Possible Breakage lists each API whose uses can break

"Possible Breakage" MUST list each removed or changed API, in the order Rust ranked it, unless no use left outside
breaks: its kind and its whole name, wrapping at its dots; under them `path:line` (a long path losing its start), the
parts that changed ("parameters and visibility changed") and the uses left outside that can break ("3 uses left
outside"), apart by `·`, none of which ends or begins a line.

#### Scenario: A removed or changed API with uses left outside
- **WHEN** the plan has a removed or changed API with uses left outside that can break
- **THEN** "Possible Breakage" lists it in Rust's order: its kind and its whole name, wrapping at its dots
- **AND** under them `path:line` (a long path losing its start), the parts that changed ("parameters and visibility changed") and the uses left outside that can break ("3 uses left outside")
- **AND** they stand apart by `·`, and a wrap leaves none at the end or the start of a line
  [@test] src/presenters/review-plan.test.ts (`lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`, `should mark in the forms of an entry what the change did: a removed parameter struck through, a changed type, an overload only one side has`)

### Requirement: In the Review Window: An entry shows its forms as code reads

An entry MUST show the form before and the form after on lines of their own, wrapping, never cut, as code reads: a
constructor as a call of its type, a line each, one not public after its visibility; each type as the source writes
it (`List<Path>`, `String?`), a wrap between the parameters or inside a type, never between a name and its type;
never the name of a `Reach` nor `List/Window`. Of a change it MUST mark what the change did, in words for a screen
reader too.

#### Scenario: The forms of a constructor
- **WHEN** an entry is a changed constructor
- **THEN** its form before and its form after stand on lines of their own, wrapping, never cut, as a call of its type, a line each: `PluginClassLoader(classPath: ClassPath, parents: Array<PluginModuleDescriptor>, libDirectories: List<Path>)`
- **AND** one not public stands after its visibility
- **AND** a wrap falls between the parameters or inside a type, never between a name and its type (`parents:` at the end of a line, `Array<PluginModuleDescriptor>` on the next)
- **AND** never the name of a `Reach` nor `List/Window`
  [@test] src/presenters/review-plan.test.ts (`lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites`, `parts a form as Rust marked it: an added or removed parameter whole, of a changed type the type alone, an overload only one side has its whole line, a name bound to its type`)
  [@test] crates/review-plan/src/api_test.rs (`a_form_shows_the_types_as_written_and_a_change_marks_its_parameters`)

#### Scenario: What a change did to the parameters
- **WHEN** an entry is a change of the parameters or the constructors
- **THEN** an added parameter is tinted in the success tone, a removed one in the destructive tone and struck through, of a changed type the type alone in the warning tone, the rest muted
- **AND** an overload only one side has is marked as a whole line
- **AND** each mark is said before its code: "added", "removed", "type changed"
  [@test] src/presenters/review-plan.test.ts (`parts a form as Rust marked it: an added or removed parameter whole, of a changed type the type alone, an overload only one side has its whole line, a name bound to its type`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`, `should mark in the forms of an entry what the change did: a removed parameter struck through, a changed type, an overload only one side has`)

### Requirement: In the Review Window: An unknown or incomplete count is a lower bound

An entry with `usages` null MUST say "The uses are unknown". An entry whose count is not `complete` MUST say "At least N
uses left outside", or "No use found outside the change; the search stopped early" at 0. An unknown, incomplete or
by-name count MUST never read as safe.

#### Scenario: The uses are unknown
- **WHEN** an entry has `usages` null, as accio did not answer
- **THEN** it stays under Possible breakage and says "The uses are unknown"
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should keep an API whose uses are unknown under Possible breakage, and say once above the map why accio did not answer`)
  [@test] src/presenters/review-plan.test.ts (`words the uses left outside, a cut or partial count as a lower bound, an unknown count as unknown`)

#### Scenario: A count that is not complete
- **WHEN** the count of an entry is not `complete`
- **THEN** it says "At least N uses left outside", or "No use found outside the change; the search stopped early" at 0
- **AND** it stays listed, never folded as safe, also at 0
  [@test] src/presenters/review-plan.test.ts (`words the uses left outside, a cut or partial count as a lower bound, an unknown count as unknown`, `folds only the API none of whose uses left outside breaks, and keeps an unknown, incomplete or by-name count listed, also at 0`)

### Requirement: In the Review Window: The card says what breaks, as a lower bound when cut

The card of a file MUST say under Breaks `breaks N` in the warning tone and why, for the uses left outside the change
of the API it removes or changes. When `breaks_complete` is false it MUST say `breaks N+` ("At least 3 uses left
outside the change; the search stopped early"), or a quiet `breaks ?` at 0, whose mark is muted: a lower bound MUST
never read as safe.

#### Scenario: A file that breaks uses outside
- **WHEN** the file removes or changes an API with uses left outside the change
- **THEN** its card says under Breaks `breaks N` in the warning tone and why
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`)
  [@test] src/presenters/review-plan.test.ts (`says the uses an API leaves outside, as a lower bound where Rust says the count is, never a safe 0`)

#### Scenario: A count that is not complete
- **WHEN** `breaks_complete` is false
- **THEN** the card says `breaks N+` ("At least 3 uses left outside the change; the search stopped early"), or a quiet `breaks ?` at 0, whose mark is muted
  [@test] src/presenters/review-plan.test.ts (`says the uses an API leaves outside, as a lower bound where Rust says the count is, never a safe 0`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should keep an API whose uses are unknown under Possible breakage, and say once above the map why accio did not answer`)

## ADDED Requirements

### Requirement: API Surface: A form shows each type as written and compares it by its names

A form MUST show the type of a Kotlin or Java parameter as its source writes it, its type arguments, `?`, a function
type, `[]` and `...` kept, annotations and runs of whitespace not; two forms MUST compare by the last names of the
types alone, so a type written in other words is no change. Of a change each parameter MUST be marked `added`,
`removed` or `typeChanged`: Kotlin by name, Java by place, an overload without a pair as a whole line.

#### Scenario: The type as written
- **WHEN** a Kotlin or a Java parameter has a type
- **THEN** its form shows it as written: `Map<String, List<T>?>`, `suspend (index: Int) -> Unit`, `String...`, `int[][]`, without an annotation
  [@test] crates/code-symbols/src/kotlin_test.rs (`a_parameter_shows_its_type_as_written`)
  [@test] crates/code-symbols/src/java_test.rs (`a_parameter_shows_its_type_as_written`)

#### Scenario: A type written in other words
- **WHEN** a commit writes a type in other words (`Map<String,List<Path>>` for `Map<String, List<Path>>`)
- **THEN** the forms are the same, and the API has no entry of it
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_parameter_of_a_form_compares_by_its_type_and_shows_it_as_written`)
  [@test] crates/review-plan/src/api_test.rs (`a_form_shows_the_types_as_written_and_a_change_marks_its_parameters`)

#### Scenario: The marks of a change
- **WHEN** a change adds, removes or retypes a parameter
- **THEN** a Kotlin parameter pairs by its name, a Java one by its place (the longest run of the same types stays), the one side only `added` or `removed`, a pair of another type `typeChanged`
- **AND** overloads pair by the most parameters they keep; one left without a pair is marked as a whole line
  [@test] crates/review-plan/src/shown_test.rs (`a_kotlin_parameter_pairs_by_its_name`, `a_java_parameter_pairs_by_its_place`, `an_overload_pairs_by_the_most_parameters_kept_and_one_left_is_marked_whole`, `the_only_constructor_of_each_side_pairs_whatever_it_keeps`)
