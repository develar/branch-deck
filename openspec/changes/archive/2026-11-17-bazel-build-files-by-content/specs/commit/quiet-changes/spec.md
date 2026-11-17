## ADDED Requirements

### Requirement: Generated: Lines inside a region a tool writes are a quiet run

The lines a change made strictly between the markers of a region a tool writes MUST be a quiet run of the kind
`generated` (`diff_marks::regions`): a marker is a comment line whose words hold `generated` (`auto-generated`,
`@generated`) and whose last word is `start` or `begin`, paired with the nearest open start of the same text ending
with `end`; an unclosed start, an end without a start and nested starts make no region. A run is the part of a block
inside a region of the same name on both sides, a side of no lines inside one when the place lies after its start
marker and up to its end marker; a marker is never in a run, a region on one side alone or renamed has none, and a
pair of no grammar has these runs too. A run of another kind that meets such a run MUST be dropped whole, and the lines
of a person outside a region keep the start of their block.

#### Scenario: A line added inside a section of a BUILD file
- **WHEN** a change adds a dependency to the section `### auto-generated section \`build a.b\` start` … `end` of a `BUILD.bazel`
- **THEN** the line is a quiet run of the kind `generated`, and its block has no start
  [@test] psi-syntax:crates/diff-marks/src/regions_test.rs (`a_line_added_inside_a_region_is_a_quiet_run_and_its_block_has_no_start`, `an_insertion_is_inside_a_region_between_its_markers`)

#### Scenario: The markers
- **WHEN** a text holds marker lines in `#`, `//` or `<!-- -->` comments, an unclosed start, an end without a start, or nested starts
- **THEN** only a start paired with its end makes a region
  [@test] psi-syntax:crates/diff-marks/src/regions_test.rs (`a_marker_is_a_comment_line_that_says_generated_and_ends_with_start_or_end`, `an_unclosed_start_an_end_without_a_start_and_nested_starts_make_no_region`, `a_line_that_is_no_marker_makes_no_region_and_lines_end_as_the_diff_ends_them`)

#### Scenario: What stays to read
- **WHEN** a change also edits a line a person wrote outside the region, crosses a marker, or has the region on one side alone or under another name
- **THEN** the line outside keeps the start of its block, only the lines inside the region are a run, and a region on one side alone or renamed has none
  [@test] psi-syntax:crates/diff-marks/src/regions_test.rs (`a_hand_written_line_outside_a_region_keeps_the_start_of_its_block`, `a_block_across_a_marker_runs_only_its_lines_inside_the_region`, `a_region_on_one_side_alone_or_renamed_has_no_run`)

#### Scenario: Another run inside a region
- **WHEN** a run of formatting or of another kind meets a run of generated lines, or the pair has no grammar
- **THEN** the run of generated lines takes the lines, the other run is dropped whole, and a pair of no grammar has the run too
  [@test] psi-syntax:crates/diff-marks/src/regions_test.rs (`a_run_of_generated_lines_takes_the_lines_of_a_run_of_formatting`, `a_run_that_meets_a_run_of_generated_lines_is_dropped_whole`, `a_pair_without_a_grammar_has_the_runs_of_its_generated_lines`)
