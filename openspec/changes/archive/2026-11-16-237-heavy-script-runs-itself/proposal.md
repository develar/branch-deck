# A script that runs itself again

## Why

A polyglot wrapper, a file that both sh and cmd.exe run, starts its sh half with
`[ -z "$BASH_VERSION" ] && exec /bin/bash "$0" "$@"`. The client read that `$0` as a script of a shell that it cannot
read, so with any `heavy` in `[host]` every run of such a wrapper was heavy, whatever it ran: a repository could not
name its wide runs heavy without queueing its narrow ones too.

## What Changes

- `$0` (`${0}`, quoted or not) as the script of a shell names what the shell names: in a script file that file, read
  already, so it adds nothing; in a sourced file the script that sources it; in the script of `-c` the word after that
  script, the `$0` around it for `"$0"`, and the name of the shell without one.
- A `$0` that names no file (the name of a shell, `$0` of the command of the run) stays a script that the client cannot
  read.
- Capability `cli/host`.
