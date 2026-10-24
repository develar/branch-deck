#!/bin/sh
# The cargo target runner of `pnpm perf:test-processes measure` (test-processes.mjs): nextest starts each test as
# `<runner> <test binary> <test name> --exact …`, and every git process of that test, a fixture's or one of the code
# under test, writes its trace2 events into a file of `$TEST_TRACE_ROOT/<binary>/<test>/`. A listing runs as it is.
binary=$1
shift
for argument in "$@"; do
  case $argument in --list) exec "$binary" "$@" ;; esac
done
name=""
for argument in "$@"; do
  case $argument in --*) ;; *) name=$argument; break ;; esac
done
directory="$TEST_TRACE_ROOT/$(basename "$binary" | sed 's/-[0-9a-f]\{16\}$//')/$(printf '%s' "$name" | tr '/' '_')"
mkdir -p "$directory"
export GIT_TRACE2_EVENT="$directory"
exec "$binary" "$@"
