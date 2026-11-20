## ADDED Requirements

### Requirement: `brd safe-push`: A lost binding refuses as `lost_binding`

With `branchdeck.symbols on`, a lost binding of what lands (a copy of a commit removes the import or the private
declaration of a name its file still uses, and its original on HEAD has no such use) MUST refuse the start with
`lost_binding` (exit 4), nothing pushed; the message MUST name the commit, the name and the use, and offer
`--allow-dangling` only when the user says it is a false alarm; `--dry-run` MUST list it in `refusals`.

#### Scenario: A lost binding
- **WHEN** a branch built without a commit that dropped a use keeps that use of an import its commit removes
- **THEN** `brd safe-push` is refused with `lost_binding` (exit 4), nothing pushed, and the message names the commit,
  the name, the use and `--allow-dangling`
- **AND** `--dry-run` lists it in `refusals`, `--allow-dangling` starts anyway
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_lost_binding_refuses_the_safe_push_unless_allowed`)
