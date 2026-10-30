// Counts the git processes of every Rust test and the time the test spends in them, so the time of the suite is judged
// by what it costs and not by the load of the machine it ran on (docs/contributing/rust-tests.md): a test's time
// follows its git processes, and what it spends beside them is the code under test and the test process itself.
//
//   pnpm perf:test-processes measure --out <dir> [-- <nextest arguments>]   e.g. -- -p sync-core, -- -E 'test(/push/)'
//   pnpm perf:test-processes summarize <out> [--top 40]
//   pnpm perf:test-processes compare <out> <other out>
//
// `measure` runs `cargo nextest run --workspace --profile ci --retries 0 --no-fail-fast` (without `--workspace` for `-p`) with `test-git-runner.sh` as the
// cargo target runner: each test process gets a trace2 directory of its own (`GIT_TRACE2_EVENT`), where every git
// process it starts writes its events, the fixture's and those of the code under test alike. It keeps `traces/`, the
// JUnit report and `run.log`, then writes `summary.json`: per test its git processes (a process git starts for itself,
// the `upload-pack` of a fetch, is not counted) by subcommand, the seconds of the test (JUnit), the seconds of its git
// processes (each from its `start` to its first `atexit`, git's own clock, `t_abs`) and the seconds that at least one
// of them ran (their union: a test that runs several at once waits for them once). What is left of the test's time
// beside that union is the test's own: its code, the start of its process and of each git (before git's clock starts),
// its waits. `summarize` prints the totals by subcommand and by test binary and the tests with the most processes and
// with the most time beside git; `compare` prints both totals and their differences. A test that traces its own git
// (`with_env("GIT_TRACE2_EVENT", …)`) writes its own file instead. macOS and Linux.
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

/** Seconds since the epoch of a trace2 `time` (`2026-10-30T04:59:02.682495Z`), to the microsecond */
function epochSeconds(time) {
  const dot = time.indexOf(".")
  return dot === -1 ? Date.parse(time) / 1000 : Date.parse(`${time.slice(0, dot)}Z`) / 1000 + Number(`0${time.slice(dot, -1)}`)
}

/**
 * The git process of one trace file, or null for a process git started for itself: `{name, start, seconds, ended}`,
 * `start` in seconds since the epoch, `seconds` from its `start` event to its first `atexit` (else `exit`, else
 * `signal`) by git's clock. A process that detaches (`maintenance --detach`) writes a second end into the same file
 * after the first, which its caller waited for. A process without an end (killed) counts to its last event.
 */
export function processOf(text) {
  let start
  const ends = {}
  let last = 0
  for (const line of text.split("\n")) {
    if (start === undefined) {
      if (line.includes('"event":"start"')) {
        const event = JSON.parse(line)
        if (event.sid.includes("/")) {
          return null
        }
        start = { name: subcommand(event.argv), at: epochSeconds(event.time) - event.t_abs, t: event.t_abs }
      }
      continue
    }
    const tAbs = /"t_abs":([0-9.]+)/.exec(line)?.[1]
    if (tAbs === undefined) {
      continue
    }
    last = Number(tAbs)
    const kind = /"event":"(atexit|exit|signal)"/.exec(line)?.[1]
    if (kind !== undefined) {
      ends[kind] ??= last
      if (kind === "atexit") {
        break
      }
    }
  }
  if (start === undefined) {
    return null
  }
  const end = ends.atexit ?? ends.exit ?? ends.signal
  return { name: start.name, start: start.at, seconds: Math.max((end ?? last) - start.t, 0), ended: end !== undefined }
}

/** The git processes of one trace2 directory, without the processes git started for itself */
export function processesOf(directory) {
  return readdirSync(directory)
    .map(file => processOf(readFileSync(path.join(directory, file), "utf8")))
    .filter(process => process !== null)
}

/** The seconds in which at least one of `processes` ran */
export function unionSeconds(processes) {
  const spans = processes.map(process => [process.start, process.start + process.seconds]).sort((a, b) => a[0] - b[0])
  let total = 0
  let [from, to] = spans[0] ?? [0, 0]
  for (const [start, end] of spans.slice(1)) {
    if (start > to) {
      total += to - from
      ;[from, to] = [start, end]
    } else {
      to = Math.max(to, end)
    }
  }
  return total + to - from
}

/** `{binary, test} -> seconds` of a nextest JUnit report; the binary and the test as the trace directories name them */
function junitTimes(file) {
  const cases = existsSync(file) ? testCases(readFileSync(file, "utf8")) : []
  return new Map(cases.map(test => [`${test.binary.replaceAll("-", "_")} ${test.name.replaceAll("/", "_")}`, test.seconds]))
}

const micro = seconds => Math.round(seconds * 1e6) / 1e6

/** The summary of a measure's `out`: per test its processes by subcommand, its seconds, in git and while git ran */
export function summaryOf(out) {
  const traces = path.join(out, "traces")
  const times = junitTimes(path.join(out, "junit.xml"))
  const tests = []
  for (const binary of readdirSync(traces)) {
    for (const test of readdirSync(path.join(traces, binary))) {
      const processes = processesOf(path.join(traces, binary, test))
      const counts = {}
      const gitSeconds = {}
      for (const { name, seconds } of processes) {
        counts[name] = (counts[name] ?? 0) + 1
        gitSeconds[name] = micro((gitSeconds[name] ?? 0) + seconds)
      }
      const junitBinary = binary === "branch_deck_lib" ? "branch_deck" : binary
      tests.push({
        binary,
        test,
        processes: processes.length,
        counts,
        seconds: times.get(`${junitBinary} ${test}`) ?? null,
        git: micro(processes.reduce((sum, process) => sum + process.seconds, 0)),
        gitWall: micro(unionSeconds(processes)),
        gitSeconds,
        unended: processes.filter(process => !process.ended).length,
      })
    }
  }
  return { tests }
}

/** The seconds of a test beside its git processes; null without its JUnit time or a summary of an older script */
export function besideGit(test) {
  return test.seconds === null || test.gitWall === undefined ? null : Math.max(test.seconds - test.gitWall, 0)
}

export function totals(summary) {
  const bySubcommand = {}
  const byBinary = {}
  const sum = { processes: 0, tests: 0, seconds: 0, git: 0, beside: 0, unended: 0, timed: summary.tests.some(test => test.gitWall !== undefined) }
  for (const test of summary.tests) {
    for (const [name, count] of Object.entries(test.counts)) {
      const subcommand = (bySubcommand[name] ??= { processes: 0, git: 0 })
      subcommand.processes += count
      subcommand.git += test.gitSeconds?.[name] ?? 0
    }
    const binary = (byBinary[test.binary] ??= { tests: 0, processes: 0, seconds: 0, git: 0, beside: 0 })
    for (const into of [binary, sum]) {
      into.tests++
      into.processes += test.processes
      into.seconds += test.seconds ?? 0
      into.git += test.git ?? 0
      into.beside += besideGit(test) ?? 0
    }
    sum.unended += test.unended ?? 0
  }
  return { ...sum, bySubcommand, byBinary }
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

const s = seconds => (seconds === null ? "-" : seconds.toFixed(1))
const ms = (seconds, count) => (count === 0 ? "-" : ((seconds / count) * 1000).toFixed(1))
const column = (text, width) => String(text).padStart(width)

/** The lines of `summarize`: the totals, by subcommand, by test binary, the costliest tests */
export function report(summary, top) {
  const sum = totals(summary)
  const lines = [`${sum.tests} tests, ${sum.processes} git processes, ${s(sum.seconds)} s of test time`]
  if (sum.timed) {
    lines.push(`  in git ${s(sum.git)} s (${ms(sum.git, sum.processes)} ms a process), beside git ${s(sum.beside)} s; ${sum.unended} processes without an end`)
  }
  lines.push("by subcommand (processes, s in git, ms a process):")
  for (const [name, { processes, git }] of Object.entries(sum.bySubcommand)
    .sort((a, b) => b[1].processes - a[1].processes)
    .slice(0, 25)) {
    lines.push(`  ${column(processes, 7)} ${column(sum.timed ? s(git) : "-", 8)} ${column(sum.timed ? ms(git, processes) : "-", 6)}  ${name}`)
  }
  lines.push("by test binary (tests, processes, test s, s in git, s beside git, test ms a process, ms in git a process):")
  for (const [name, binary] of Object.entries(sum.byBinary).sort((a, b) => b[1].seconds - a[1].seconds || b[1].processes - a[1].processes)) {
    const [git, beside, gitMs] = sum.timed ? [s(binary.git), s(binary.beside), ms(binary.git, binary.processes)] : ["-", "-", "-"]
    lines.push(
      `  ${column(binary.tests, 5)} ${column(binary.processes, 7)} ${column(s(binary.seconds), 8)} ${column(git, 8)} ${column(beside, 8)} ${column(ms(binary.seconds, binary.processes), 6)} ${column(gitMs, 6)}  ${name}`,
    )
  }
  const testLine = test =>
    `${column(test.processes, 5)} ${column(s(test.seconds), 6)} ${column(s(test.git ?? null), 6)} ${column(s(besideGit(test)), 6)}  ${test.binary} ${test.test}`
  lines.push(`the ${top} tests with the most processes (processes, test s, s in git, s beside git):`)
  lines.push(
    ...[...summary.tests]
      .sort((a, b) => b.processes - a.processes)
      .slice(0, top)
      .map(test => `  ${testLine(test)}`),
  )
  if (sum.timed) {
    lines.push(`the ${top} tests with the most time beside git (processes, test s, s in git, s beside git):`)
    lines.push(
      ...[...summary.tests]
        .sort((a, b) => (besideGit(b) ?? 0) - (besideGit(a) ?? 0))
        .slice(0, top)
        .map(test => `  ${testLine(test)}`),
    )
  }
  return lines
}

function print(summary, top) {
  console.log(report(summary, top).join("\n"))
}

/** The lines of `compare`: the totals of both, the subcommands and the binaries whose numbers differ */
export function comparison(first, second) {
  const [a, b] = [totals(first), totals(second)]
  const change = (x, y) => (x === 0 ? "" : ` (${(((y - x) / x) * 100).toFixed(1)}%)`)
  const lines = [`git processes: ${a.processes} → ${b.processes}${change(a.processes, b.processes)}, tests ${a.tests} → ${b.tests}`]
  lines.push(`test s: ${s(a.seconds)} → ${s(b.seconds)}${change(a.seconds, b.seconds)}`)
  if (a.timed && b.timed) {
    lines.push(`  in git ${s(a.git)} → ${s(b.git)} s, ${ms(a.git, a.processes)} → ${ms(b.git, b.processes)} ms a process; beside git ${s(a.beside)} → ${s(b.beside)} s`)
  }
  const names = new Set([...Object.keys(a.bySubcommand), ...Object.keys(b.bySubcommand)])
  const rows = [...names].map(name => [name, a.bySubcommand[name]?.processes ?? 0, b.bySubcommand[name]?.processes ?? 0]).filter(([, x, y]) => x !== y)
  for (const [name, x, y] of rows.sort((p, q) => Math.abs(q[2] - q[1]) - Math.abs(p[2] - p[1])).slice(0, 25)) {
    lines.push(`  ${column(x, 7)} → ${column(y, 7)}  ${name}`)
  }
  const empty = { tests: 0, processes: 0, seconds: 0, git: 0, beside: 0 }
  const binaries = [...new Set([...Object.keys(a.byBinary), ...Object.keys(b.byBinary)])].map(name => [name, a.byBinary[name] ?? empty, b.byBinary[name] ?? empty])
  lines.push(`by test binary (tests, processes, test s${a.timed && b.timed ? ", s in git, s beside git" : ""}: first → second):`)
  for (const [name, x, y] of binaries.sort((p, q) => Math.abs(q[2].seconds - q[1].seconds) - Math.abs(p[2].seconds - p[1].seconds))) {
    const timed = a.timed && b.timed ? `; ${s(x.git)} → ${s(y.git)}; ${s(x.beside)} → ${s(y.beside)}` : ""
    lines.push(`  ${x.tests} → ${y.tests}; ${x.processes} → ${y.processes}; ${s(x.seconds)} → ${s(y.seconds)}${timed}  ${name}`)
  }
  return lines
}

if (import.meta.main) {
  const [command, ...args] = process.argv.slice(2)
  if (command === "measure") {
    measure(args)
  } else if (command === "summarize") {
    const top = args.includes("--top") ? Number(args[args.indexOf("--top") + 1]) : 40
    print(load(args[0]), top)
  } else if (command === "compare") {
    console.log(comparison(load(args[0]), load(args[1])).join("\n"))
  } else {
    console.error("usage: test-processes.mjs measure --out <dir> [-- <nextest arguments>] | summarize <out> [--top n] | compare <out> <other>")
    process.exitCode = 2
  }
}
