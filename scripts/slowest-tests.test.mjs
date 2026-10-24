import assert from "node:assert/strict"
import { test } from "node:test"
import { summary, testCases } from "./slowest-tests.mjs"

const REPORT = `<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="0" errors="0" time="9.5">
  <testsuite name="sync-core" tests="2" disabled="0" errors="0" failures="0">
    <testcase name="sync_test::a_sync" classname="sync-core" timestamp="2026-10-24T17:00:00.000Z" time="1.25">
    </testcase>
    <testcase name="push_test::a_push_&amp;_more" classname="sync-core" timestamp="2026-10-24T17:00:00.000Z" time="6.5">
      <system-out>…</system-out>
    </testcase>
  </testsuite>
  <testsuite name="bd-remote" tests="1" disabled="0" errors="0" failures="0">
    <testcase name="scenario_test::the_cases" classname="bd-remote" timestamp="2026-10-24T17:00:00.000Z" time="2"/>
  </testsuite>
</testsuites>
`

test("the cases of a report are read with their binary and their seconds", () => {
  assert.deepEqual(testCases(REPORT), [
    { binary: "sync-core", name: "sync_test::a_sync", seconds: 1.25 },
    { binary: "sync-core", name: "push_test::a_push_&_more", seconds: 6.5 },
    { binary: "bd-remote", name: "scenario_test::the_cases", seconds: 2 },
  ])
})

test("the summary lists the slowest tests first, then the binaries by their time", () => {
  assert.equal(
    summary(testCases(REPORT), 2),
    [
      "### Rust tests: 3 tests, 10 s of test time",
      "",
      "| Test | Binary | s |",
      "|---|---|---:|",
      "| `push_test::a_push_&_more` | sync-core | 6.5 |",
      "| `scenario_test::the_cases` | bd-remote | 2.0 |",
      "",
      "| Binary | Tests | s |",
      "|---|---:|---:|",
      "| sync-core | 2 | 8 |",
      "| bd-remote | 1 | 2 |",
      "",
    ].join("\n"),
  )
})
