// The slowest tests of a nextest JUnit report and the time of each test binary, as Markdown: the CI job summary of the
// Rust tests shows them, so a test that became slow is seen in the run that made it slow, on each platform. It fails
// nothing: the time of a test depends on the runner's load; the process count of a fixture is pinned by a test
// (`a_fixture_starts_one_git_process_per_step`).
// Run: `node scripts/slowest-tests.mjs target/nextest/ci/junit.xml [count] >> "$GITHUB_STEP_SUMMARY"`.
import { existsSync, readFileSync } from "node:fs"

/** `{binary, name, seconds}` of every test case of the report, in the order of the report */
export function testCases(xml) {
  const cases = []
  const suite = /<testsuite\b[^>]*\bname="([^"]*)"[^>]*>([\s\S]*?)<\/testsuite>/g
  for (const [, binary, body] of xml.matchAll(suite)) {
    for (const [attributes] of body.matchAll(/<testcase\b[^>]*>/g)) {
      const name = /\bname="([^"]*)"/.exec(attributes)?.[1]
      const time = /\btime="([^"]*)"/.exec(attributes)?.[1]
      if (name !== undefined && time !== undefined) {
        cases.push({ binary: unescape(binary), name: unescape(name), seconds: Number(time) })
      }
    }
  }
  return cases
}

function unescape(text) {
  return text.replaceAll("&lt;", "<").replaceAll("&gt;", ">").replaceAll("&quot;", '"').replaceAll("&apos;", "'").replaceAll("&amp;", "&")
}

/** The Markdown of the `count` slowest tests and of the time per binary */
export function summary(cases, count) {
  const total = cases.reduce((sum, test) => sum + test.seconds, 0)
  const lines = [`### Rust tests: ${cases.length} tests, ${total.toFixed(0)} s of test time`, "", `| Test | Binary | s |`, `|---|---|---:|`]
  for (const test of [...cases].sort((a, b) => b.seconds - a.seconds).slice(0, count)) {
    lines.push(`| \`${test.name}\` | ${test.binary} | ${test.seconds.toFixed(1)} |`)
  }
  const binaries = new Map()
  for (const test of cases) {
    const binary = binaries.get(test.binary) ?? { seconds: 0, tests: 0 }
    binary.seconds += test.seconds
    binary.tests += 1
    binaries.set(test.binary, binary)
  }
  lines.push("", `| Binary | Tests | s |`, `|---|---:|---:|`)
  for (const [name, binary] of [...binaries].sort((a, b) => b[1].seconds - a[1].seconds)) {
    lines.push(`| ${name} | ${binary.tests} | ${binary.seconds.toFixed(0)} |`)
  }
  return `${lines.join("\n")}\n`
}

if (import.meta.main) {
  const [path, count = "30"] = process.argv.slice(2)
  if (path === undefined || !existsSync(path)) {
    console.log(`No JUnit report at ${path}: the tests did not run`)
  } else {
    process.stdout.write(summary(testCases(readFileSync(path, "utf8")), Number(count)))
  }
}
