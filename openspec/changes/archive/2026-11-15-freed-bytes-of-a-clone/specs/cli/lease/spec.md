## MODIFIED Requirements

### Requirement: Clean rules: The trash is deleted once the slots are free

Once the slot is free the return, `brd lease clean` once every slot it marked is free again, and `brd lease reclaim`
once every slot it reclaimed is free, MUST delete their trash, counting `freed_bytes` in the same walk, and the trash of
a run whose process is gone; a path that can't be deleted stays and is named in `trash_left`. `freed_bytes` MUST
count what each unlink frees, never more than the volume gains.

#### Scenario: The trash of a gone run
- **WHEN** the trash of a run whose process is gone is in the pool
- **THEN** the return deletes it with its own
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_return_deletes_through_the_trash_and_what_a_gone_run_left_there`)

#### Scenario: The bytes freed
- **WHEN** the trash is deleted
- **THEN** an unlink counts nothing while another hard link keeps the file, else on macOS the private size of the file
  (`ATTR_CMNEXT_PRIVATESIZE`, read once a directory by `getattrlistbulk` before its deletes), else its blocks
- **AND** `brd lease clean` also prints `volume_freed_bytes`, what the volume gained (`statfs` before and after)
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_clone_frees_only_the_blocks_it_shares_with_no_other_file`, `a_hard_link_frees_nothing_while_another_link_keeps_the_file`)
