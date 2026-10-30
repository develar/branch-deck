import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { test } from "node:test"
import { besideGit, binaryFileName, comparison, processOf, report, subcommand, summaryOf, totals, unionSeconds } from "./test-processes.mjs"

const SID = "20261030T045902.682112Z-H6953c50e-P00007c43"

/** The trace2 lines of one git process: its start at `time` (`t_abs` 0.0005), then `events` as `[event, t_abs]` */
function trace(sid, argv, time, events) {
  const start = { event: "start", sid, thread: "main", time, file: "common-init.c", line: 78, t_abs: 0.0005, argv }
  const lines = [{ event: "version", sid, thread: "main", time, evt: "4", exe: "2.56.0" }, start]
  for (const [event, tAbs] of events) {
    lines.push(event === "cmd_name" ? { event, sid, thread: "main", time, name: argv[1] } : { event, sid, thread: "main", time, t_abs: tAbs, code: 0 })
  }
  return `${lines.map(line => JSON.stringify(line)).join("\n")}\n`
}

test("the subcommand is the first word after the options of git", () => {
  assert.equal(subcommand(["git", "-c", "core.abbrev=no", "-C", "/repo", "--no-optional-locks", "log", "-1"]), "log")
  assert.equal(subcommand(["git", "--version"]), "")
})

test("a process lasts from its start to its first atexit by git's clock, and starts that much before its start event", () => {
  const process = processOf(trace(SID, ["git", "log"], "2026-10-30T04:59:02.682495Z", [["cmd_name"], ["data", 0.0017], ["exit", 0.0119], ["atexit", 0.012]]))
  assert.equal(process.name, "log")
  assert.equal(process.ended, true)
  assert.ok(Math.abs(process.seconds - 0.0115) < 1e-9)
  assert.ok(Math.abs(process.start - (Date.UTC(2026, 9, 30, 4, 59, 2) / 1000 + 0.682495 - 0.0005)) < 1e-6)
})

test("a process that detaches ends at its first end, a killed one at its last event, a child of git is none", () => {
  // `maintenance --detach`: the parent ends at 3 ms, the detached child writes its own end at 9 ms into the same file
  const detached = processOf(
    trace(SID, ["git", "maintenance"], "2026-10-30T04:59:02.000000Z", [
      ["exit", 0.0035],
      ["atexit", 0.0036],
      ["exit", 0.009],
      ["atexit", 0.0091],
    ]),
  )
  assert.ok(Math.abs(detached.seconds - 0.0031) < 1e-9)
  const killed = processOf(trace(SID, ["git", "cat-file"], "2026-10-30T04:59:02.000000Z", [["data", 0.25]]))
  assert.equal(killed.ended, false)
  assert.ok(Math.abs(killed.seconds - 0.2495) < 1e-9)
  const signaled = processOf(trace(SID, ["git", "fetch"], "2026-10-30T04:59:02.000000Z", [["signal", 0.0405]]))
  assert.equal(signaled.ended, true)
  assert.ok(Math.abs(signaled.seconds - 0.04) < 1e-9)
  assert.equal(processOf(trace(`${SID}/20261030T045902.697466Z-H6953c50e-P00007c51`, ["git-upload-pack", "/repo"], "2026-10-30T04:59:02.000000Z", [["atexit", 0.01]])), null)
})

test("a binary of the report is named as its file: the crate of its unit tests, else its target", () => {
  assert.equal(binaryFileName("bd-remote"), "bd_remote")
  assert.equal(binaryFileName("host-agent::serve"), "serve")
  assert.equal(binaryFileName("bd-host::bin/bd-host"), "bd_host")
})

test("the processes that run at once are counted once in the time git ran", () => {
  const span = (start, seconds) => ({ start, seconds })
  assert.equal(unionSeconds([]), 0)
  assert.equal(unionSeconds([span(10, 1), span(10.5, 1), span(13, 0.5), span(10.2, 0.1)]), 2)
  assert.equal(unionSeconds([span(5, 1), span(1, 1)]), 2)
})

const JUNIT = `<testsuites>
  <testsuite name="bd-remote" tests="2">
    <testcase name="push_test::a_push" classname="bd-remote" time="1.5"/>
    <testcase name="scenario_test::the_cases" classname="bd-remote" time="3"/>
  </testsuite>
</testsuites>
`

/** An `out` of a measure: two tests of bd-remote, one with two git processes at once and one after them */
function measured() {
  const out = mkdtempSync(path.join(tmpdir(), "test-processes-"))
  const write = (test, file, text) => {
    const directory = path.join(out, "traces", "bd_remote", test)
    mkdirSync(directory, { recursive: true })
    writeFileSync(path.join(directory, file), text)
  }
  write("push_test::a_push", "1", trace(`${SID}1`, ["git", "log"], "2026-10-30T05:00:00.000500Z", [["atexit", 0.4005]]))
  write("push_test::a_push", "2", trace(`${SID}2`, ["git", "log"], "2026-10-30T05:00:00.200500Z", [["atexit", 0.4005]]))
  write("push_test::a_push", "3", trace(`${SID}3`, ["git", "push"], "2026-10-30T05:00:01.000500Z", [["atexit", 0.1005]]))
  write("scenario_test::the_cases", "1", trace(`${SID}4`, ["git", "status"], "2026-10-30T05:00:00.000500Z", [["atexit", 1.0005]]))
  writeFileSync(path.join(out, "junit.xml"), JUNIT)
  return out
}

test("a test's time is split into its time in git, while at least one ran, and beside git", t => {
  const out = measured()
  t.after(() => rmSync(out, { recursive: true, force: true }))
  const summary = summaryOf(out)
  const push = summary.tests.find(test => test.test === "push_test::a_push")
  assert.deepEqual(push.counts, { log: 2, push: 1 })
  assert.equal(push.seconds, 1.5)
  assert.equal(push.git, 0.9)
  assert.equal(push.gitWall, 0.7)
  assert.ok(Math.abs(besideGit(push) - 0.8) < 1e-9)
  assert.deepEqual(push.gitSeconds, { log: 0.8, push: 0.1 })

  const sum = totals(summary)
  assert.deepEqual([sum.tests, sum.processes, sum.seconds, sum.unended], [2, 4, 4.5, 0])
  assert.ok(Math.abs(sum.git - 1.9) < 1e-9 && Math.abs(sum.beside - 2.8) < 1e-9)
  assert.equal(sum.bySubcommand.log.processes, 2)
  assert.deepEqual(sum.byBinary.bd_remote.tests, 2)
  assert.ok(Math.abs(sum.byBinary.bd_remote.beside - 2.8) < 1e-9)
})

test("summarize prints the time beside git by binary and the tests with the most of it, compare both runs", t => {
  const out = measured()
  t.after(() => rmSync(out, { recursive: true, force: true }))
  const summary = summaryOf(out)
  const lines = report(summary, 1)
  assert.equal(lines[0], "2 tests, 4 git processes, 4.5 s of test time")
  assert.equal(lines[1], "  in git 1.9 s (475.0 ms a process), beside git 2.8 s; 0 processes without an end")
  assert.ok(lines.includes("      2       4      4.5      1.9      2.8 1125.0  475.0  bd_remote"))
  assert.deepEqual(lines.slice(-2), [
    "the 1 tests with the most time beside git (processes, test s, s in git, s beside git):",
    "      1    3.0    1.0    2.0  bd_remote scenario_test::the_cases",
  ])

  // a summary of the script before the times of git: its counts compare, its times are none
  const untimed = { tests: summary.tests.map(({ binary, test, processes, counts, seconds }) => ({ binary, test, processes, counts, seconds })) }
  assert.equal(report(untimed, 1)[1], "by subcommand (processes, s in git, ms a process):")
  assert.deepEqual(comparison(untimed, summary).slice(0, 2), ["git processes: 4 → 4 (0.0%), tests 2 → 2", "test s: 4.5 → 4.5 (0.0%)"])
  assert.equal(comparison(summary, summary)[2], "  in git 1.9 → 1.9 s, 475.0 → 475.0 ms a process; beside git 2.8 → 2.8 s")
})
