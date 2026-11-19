# Proposal

## Why

On 2026-11-18 and 19 every run of branch-deck and accio on the Studio hung in its checkout for 13 to 50 minutes right
after each upgrade of the agent: their pools were on `/Volumes/ssd`, and tccd found no Full Disk Access for the new
ad-hoc signed `~/.local/bin/brd-host` ("Failed to match existing code requirement … kTCCServiceSystemPolicyAllFiles"),
so `open` in the slot waited on a consent that a `LaunchAgent` cannot show. Nothing told the person who upgraded.

## What Changes

- After an upgrade or a setup of a macOS agent, the client asks the lend tables and warns of each slot below
  `/Volumes/`, with the two ways out.
