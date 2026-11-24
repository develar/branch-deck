# Proposal

## Why

A heavy run without a measure takes the whole host, and the key of its measures is the text of its command. A gate
that runs as `bash -c "$(cat gate.sh)" … <base> <tip>` has its commits and paths in its words, so every run has a new
key, never gets a measure, and waits for an empty host: over 7 days such commands waited 7.7 h on the Studio while
they used 3.6 to 7.7 of its 20 cores.

## What Changes

- `brd-host run --measure-as <class>`: a heavy run whose measures are those of the class, not of its command. Its first
  run takes the whole host and is measured; the later runs of the class share the cores by that measure.
- `RunSpec::measure_as` and `EstimateAsk::measure_as`, behind the feature `measure_as`; the client refuses the run on an
  older agent, and a member of a set without the feature has no estimate of it.
- `host_link::heavy::measure_key` takes the class: the admission, the record, the estimate, the queue record and the
  history take one key.
- The line of a wait without a measure says that `--measure-as <class>` gives one to a command whose text changes;
  `brd-host runs` names the class of a waiting run, and `brd-host stats` names a class instead of its command lines.

Capabilities: `cli/host`.
