## ADDED Requirements

### Requirement: Starting a Safe Push or a Dry Run: A lost binding refuses the start

With `branchdeck.symbols on`, the landing gate MUST refuse a start whose built copy of a commit removes the import or
the `private` declaration of a name that its file in the tree that lands still uses, while the original of the commit
on HEAD has no such use (`LandingRefusal::LostBinding`, `trial_check::lost_bindings`): git merges it, the build fails.
No branch fixes it, so it MUST offer only Start Anyway, which lifts it as a dangling reference.

#### Scenario: A branch built without the commit that dropped a use
- **GIVEN** a commit of one branch drops a use of an import, and a commit of another branch, later on HEAD, drops its
  last use and the import
- **WHEN** a start lands the second branch alone
- **THEN** the gate refuses it with a lost binding of the import at its use in the tree that lands
- **AND** a dangling reference of the same name in the same file is the refusal instead, with its fix
  [@test] crates/trial-check/src/landing_gate_test.rs (`a_commit_whose_branch_keeps_a_use_of_what_it_removes_refuses_the_start`)

#### Scenario: The words of a lost binding
- **WHEN** the gate refuses a lost binding
- **THEN** the reason says "“IJAI-6702 a local resume stops an idle job” of claude-fork-open removes the import of
  rethrowControlFlowException, which ClaudePromptLaunchSupport.kt:139 still uses: git merges it, but the build of
  claude-fork-open fails. Keep it in that commit."
- **AND** the row says "Needs fix of 6a4e00db", and Start Anyway lifts it with `allowDangling`
  [@test] src/presenters/landing.test.ts (`refuses a commit that removes the import of a name its file still uses in the tree that lands, which only that commit fixes`)

#### Scenario: A name that resolves another way
- **WHEN** a commit removes a binding where the name still resolves (an extension called on a receiver, a default
  import, an import of the own package, a wider declaration, a local of the same name), or its original on HEAD has the
  same use without the binding
- **THEN** nothing refuses the start
  [@test] crates/trial-check/src/lost_bindings_test.rs (`a_qualified_use_or_an_extension_call_is_no_use_of_the_binding`, `an_import_of_a_default_or_the_own_package_binds_nothing_the_file_lacks`, `a_public_member_or_an_override_may_resolve_another_way`, `a_local_of_the_same_name_takes_the_use`, `a_name_still_imported_another_way_is_not_lost`)

#### Scenario: The rule of one file
- **WHEN** the file before a commit binds a name by an import or a private declaration, and the file after it binds
  it by nothing but still uses it bare
- **THEN** the name is lost, with the line of its binding and the first line of its use
  [@test] crates/trial-check/src/lost_bindings_test.rs (`the_replayed_commit_of_the_incident_loses_the_property_and_the_import`, `a_type_whose_import_goes_is_lost`, `java_loses_a_private_member_and_an_import_but_not_a_wider_member`, `a_commit_that_removes_the_uses_too_loses_nothing`, `a_declaration_that_stays_with_another_reach_is_not_lost`, `a_degraded_side_tells_nothing`, `the_lost_bindings_of_commits_in_two_processes`)
