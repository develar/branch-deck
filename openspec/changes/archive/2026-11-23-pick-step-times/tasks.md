# Tasks

- [x] The core tells each step of a pick to its caller as it starts (`PickStep`, `pick_core`'s `progress`)
- [x] `brd pick` and `brd lease pick` print each step on stderr with its time and list them in `steps`
- [x] The update of a lease after a conflict is a step of `brd lease pick`
- [x] Tests of the steps of a pick, a dry run, a lease pick that conflicts, and of the line
- [x] "Pick" in docs/design/git-operations.md and `brd pick` in docs/contributing/agent-cli.md name the steps
