# Tasks

- [x] The rule `crates/domain-policy/src/landed_island.rs` with its table test
- [x] The session keeps the sightings, hides after its successful sync under the lease it opened under, keeps them across
  a reopen of a session that knew no lease (`crates/repository-model/src/session_test.rs`)
- [x] The command `landed_seen` in place of `hide_landed`, the bindings and `commands.json`
- [x] The page reports the sighting only (`src/stores/landed.ts`, `LandedCard.tsx`); the e2e moves the clock of the session
- [x] The test links of every scenario, `pnpm lint:specs`
