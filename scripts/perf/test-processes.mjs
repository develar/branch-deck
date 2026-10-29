// Counts the git processes of every Rust test, so the time of the suite is judged by what it costs and not by the load of
// the machine it ran on (docs/contributing/rust-tests.md): a test's time follows its git processes.
//
//   pnpm perf:test-processes measure --out <dir> [-- <nextest arguments>]   e.g. -- -p sync-core, -- -E 'test(/push/)'
//   pnpm perf:test-processes summarize <out> [--top 40]
//   pnpm perf:test-processes compare <out> <other out>
//
// `measure` runs `cargo nextest run --workspace --profile ci --retries 0 --no-fail-fast` (without `--workspace` for `-p`) with `test-git-runner.sh` as the
// cargo target runner: each test process gets a trace2 directory of its own (`GIT_TRACE2_EVENT`), where every git
// process it starts writes its events, the fixture's and those of the code under test alike. It keeps `traces/`, the
// JUnit report and `run.log`, then writes `summary.json`: per test its git processes (a process git starts for itself,
// the `upload-pack` of a fetch, is not counted) by subcommand and its time. `summarize` prints the totals by subcommand
// and by test binary and the tests with the most processes; `compare` prints both totals and their differences.
// A test that traces its own git (`with_env("GIT_TRACE2_EVENT", …)`) writes its own file instead. macOS and Linux.
import { execFileSync, spawnSync } from "node:child_process"
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import { testCases } from "../slowest-tests.mjs"

const root = path.join(import.meta.dirname, "..", "..")

/** The subcommand of a git command line: the first word after the program and its options, `-c name=value` included */
export function subcommand(argv) {
  for (let index = 1; index < argv.length; index++) {
    const word = argv[index]
    if (word === "-c" || word === "-C") {
      index++
    } else if (!word.startsWith("-")) {
      return word
    }
  }
  return ""
}

/** The subcommands of the git processes of one trace2 directory, without the processes git started for itself */
export function processesOf(directory) {
  const subcommands = []
  for (const file of readdirSync(directory)) {
    for (const line of readFileSync(path.join(directory, file), "utf8").split("\n")) {
      if (!line.includes('"event":"start"')) {
        continue
      }
      const event = JSON.parse(line)
      if (!event.sid.includes("/")) {
        subcommands.push(subcommand(event.argv))
      }
      break
    }
  }
  return subcommands
}

/** `{binary, test} -> seconds` of a nextest JUnit report; the binary and the test as the trace directories name them */
function junitTimes(file) {
  const cases = existsSync(file) ? testCases(readFileSync(file, "utf8")) : []
  return new Map(cases.map(test => [`${test.binary.replaceAll("-", "_")} ${test.name.replaceAll("/", "_")}`, test.seconds]))
}

function summaryOf(out) {
  const traces = path.join(out, "traces")
  const times = junitTimes(path.join(out, "junit.xml"))
  const tests = []
  for (const binary of readdirSync(traces)) {
    for (const test of readdirSync(path.join(traces, binary))) {
      const counts = {}
      for (const name of processesOf(path.join(traces, binary, test))) {
        counts[name] = (counts[name] ?? 0) + 1
      }
      const junitBinary = binary === "branch_deck_lib" ? "branch_deck" : binary
      tests.push({ binary, test, processes: Object.values(counts).reduce((a, b) => a + b, 0), counts, seconds: times.get(`${junitBinary} ${test}`) ?? null })
    }
  }
  return { tests }
}

function totals(summary) {
  const bySubcommand = {}
  const byBinary = {}
  for (const test of summary.tests) {
    for (const [name, count] of Object.entries(test.counts)) {
      bySubcommand[name] = (bySubcommand[name] ?? 0) + count
    }
    const binary = (byBinary[test.binary] ??= { tests: 0, processes: 0 })
    binary.tests++
    binary.processes += test.processes
  }
  return { processes: summary.tests.reduce((sum, test) => sum + test.processes, 0), tests: summary.tests.length, bySubcommand, byBinary }
}

function measure(args) {
  const separator = args.indexOf("--")
  const own = separator === -1 ? args : args.slice(0, separator)
  const nextestArgs = separator === -1 ? [] : args.slice(separator + 1)
  const out = own[own.indexOf("--out") + 1]
  if (!own.includes("--out") || out === undefined) {
    throw new Error("measure needs --out <dir>")
  }
  const traces = path.resolve(out, "traces")
  mkdirSync(traces, { recursive: true })
  const host = /^host: (\S+)$/m.exec(execFileSync("rustc", ["-vV"], { cwd: root, encoding: "utf8" }))[1]
  const runner = `CARGO_TARGET_${host.toUpperCase().replaceAll("-", "_")}_RUNNER`
  // `--workspace` beside `-p` would run every package
  const packages = nextestArgs.some(arg => arg === "-p" || arg.startsWith("--package"))
  const nextest = ["nextest", "run", ...(packages ? [] : ["--workspace"]), "--profile", "ci", "--retries", "0", "--no-fail-fast", ...nextestArgs]
  console.error(`cargo ${nextest.join(" ")} with ${runner}`)
  const log = path.join(out, "run.log")
  const started = Date.now()
  const result = spawnSync("cargo", nextest, {
    cwd: root,
    env: { ...process.env, [runner]: path.join(import.meta.dirname, "test-git-runner.sh"), TEST_TRACE_ROOT: traces },
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 1 << 30,
  })
  writeFileSync(log, `${result.stdout}${result.stderr}`)
  const junit = path.join(root, "target", "nextest", "ci", "junit.xml")
  if (existsSync(junit)) {
    copyFileSync(junit, path.join(out, "junit.xml"))
  }
  const summary = summaryOf(out)
  writeFileSync(path.join(out, "summary.json"), `${JSON.stringify(summary)}\n`)
  console.error(`exit ${result.status} after ${((Date.now() - started) / 1000).toFixed(0)} s; ${log}`)
  print(summary, 20)
  process.exitCode = result.status ?? 1
}

/** `summary.json` of `out`, written from its traces first when a measure was cut short */
function load(out) {
  const file = path.join(out, "summary.json")
  if (!existsSync(file)) {
    writeFileSync(file, `${JSON.stringify(summaryOf(out))}\n`)
  }
  return JSON.parse(readFileSync(file, "utf8"))
}

function print(summary, top) {
  const { processes, tests, bySubcommand, byBinary } = totals(summary)
  console.log(`${tests} tests, ${processes} git processes`)
  console.log("by subcommand:")
  for (const [name, count] of Object.entries(bySubcommand)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 25)) {
    console.log(`  ${String(count).padStart(7)}  ${name}`)
  }
  console.log("by test binary (tests, processes, per test):")
  for (const [name, binary] of Object.entries(byBinary).sort((a, b) => b[1].processes - a[1].processes)) {
    console.log(`  ${String(binary.tests).padStart(5)} ${String(binary.processes).padStart(7)} ${(binary.processes / binary.tests).toFixed(1).padStart(6)}  ${name}`)
  }
  console.log(`the ${top} tests with the most processes (processes, s):`)
  for (const test of [...summary.tests].sort((a, b) => b.processes - a.processes).slice(0, top)) {
    console.log(`  ${String(test.processes).padStart(5)} ${String(test.seconds?.toFixed(1) ?? "-").padStart(6)}  ${test.binary} ${test.test}`)
  }
}

function compare(first, second) {
  const [a, b] = [totals(load(first)), totals(load(second))]
  console.log(`git processes: ${a.processes} → ${b.processes} (${(((b.processes - a.processes) / a.processes) * 100).toFixed(1)}%), tests ${a.tests} → ${b.tests}`)
  const names = new Set([...Object.keys(a.bySubcommand), ...Object.keys(b.bySubcommand)])
  const rows = [...names].map(name => [name, a.bySubcommand[name] ?? 0, b.bySubcommand[name] ?? 0]).filter(([, x, y]) => x !== y)
  for (const [name, x, y] of rows.sort((p, q) => Math.abs(q[2] - q[1]) - Math.abs(p[2] - p[1])).slice(0, 25)) {
    console.log(`  ${String(x).padStart(7)} → ${String(y).padStart(7)}  ${name}`)
  }
}

if (import.meta.main) {
  const [command, ...args] = process.argv.slice(2)
  if (command === "measure") {
    measure(args)
  } else if (command === "summarize") {
    const top = args.includes("--top") ? Number(args[args.indexOf("--top") + 1]) : 40
    print(load(args[0]), top)
  } else if (command === "compare") {
    compare(args[0], args[1])
  } else {
    console.error("usage: test-processes.mjs measure --out <dir> [-- <nextest arguments>] | summarize <out> [--top n] | compare <out> <other>")
    process.exitCode = 2
  }
}
