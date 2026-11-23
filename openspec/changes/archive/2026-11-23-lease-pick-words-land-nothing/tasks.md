# Tasks

- [x] `lease_pick_reporting` stops after an update whose words resolved a conflict, before the gate, with a
      `conflict.next` that names the files (`words_next`)
- [x] Tests: an update resolved by words lands nothing and names the file, then the next pick lands it; an update that
      drops the conflicting commit as landed lands in one call; the gated test reaches its gate by such a drop
- [x] `skill_text()`, "Pick" of docs/design/git-operations.md, docs/contributing/leases.md, the doc of
      `LeasePickOutput::update`
