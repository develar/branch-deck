## ADDED Requirements

### Requirement: Answers: The places are answered within a number of resolutions

A reader MUST be able to ask the names of many places within a number of resolutions of a name: one per spelling
asked and one more per binding asked with another name, in the order of the questions. A place whose answer needs a
resolution past them MUST have no answer, and the answers within the limit MUST be those without one. What a
resolution costs MUST be a part of the tokens of its text by its grammar, the same on every machine.

#### Scenario: Two locals of one spelling and two other names
- **WHEN** two places of a local, a place of another local of its spelling, a name no local binds and a parameter are asked, the locals with a new name, within zero to five resolutions
- **THEN** five resolutions answer every place, as without a limit; one resolution, the spelling alone, answers no local; each one more answers the next binding or spelling asked, and a later place of a binding answered before
- **AND** a resolution costs the tokens of the text in Java, C# and Starlark, an eighth of them in Kotlin, Rust and TypeScript
  [@test] crates/code-symbols/src/names_at_test.rs (`the_places_are_answered_within_a_number_of_resolutions`)
