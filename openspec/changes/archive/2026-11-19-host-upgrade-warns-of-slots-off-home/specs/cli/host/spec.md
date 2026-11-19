## ADDED Requirements

### Requirement: upgrade: upgrade and setup warn of slots off the home volume of a macOS agent

After an upgrade or a setup of a macOS agent, the client MUST ask the lend tables of the host (`Request::Slots`), and
when a slot of a repository is below `/Volumes/`, on another volume than the home of the agent, it MUST say on stderr
`brd-host: warning: slots on <alias> are on another volume than the home of the agent: <repo> (<paths>); …`, with the
two ways out: the pools on the home volume (`git config --unset branchdeck.leasePool` in the repository on the host), or
Full Disk Access given again to `~/.local/bin/brd-host`. macOS keeps Full Disk Access for the code identity of a binary,
and the ad-hoc signed agent is a new identity after each upgrade, so its runs on such a volume hang in their checkout
(in `open`, on a consent that a `LaunchAgent` cannot show) until the access is given again. A failed look says nothing:
the upgrade or the setup went through.

#### Scenario: A slot on another volume
- **WHEN** an upgrade or a setup of a macOS agent ends, and a repository of the host has a slot below `/Volumes/`
- **THEN** stderr gets the warning with the repository, its slots there and the two ways out
  [@test] crates/host-agent/src/table_test.rs (`the_slots_off_the_home_volume_get_the_warning_of_an_upgrade`)

#### Scenario: Every slot on the home volume
- **WHEN** every slot of the host is on the home volume
- **THEN** the upgrade and the setup say no warning
  [@test] crates/host-agent/src/table_test.rs (`the_slots_off_the_home_volume_get_the_warning_of_an_upgrade`)
