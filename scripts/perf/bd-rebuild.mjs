// Measures the rebuild of the bd crates after a one-line change, so the cut of `crates/bd-cli` into crates is judged by
// numbers.
//
//   pnpm perf:bd-rebuild measure --out <dir> [--runs 5] [--edits safe_push,clustering,lease,sync] [--workspace] [--clean] [--phases]
//
// The edits (`EDITS`): `safe_push`, `clustering`, `lease`, `sync`, `check_command` and `update_command` change a bd
// command; `remote` (`list_pull_requests`), `check` (`check_plan`), `update` (`update_project_core`) and `lease_pool`
// (`pool_dir`) change a core of sync-core, so the split of sync-core is measured by the same edit before and after.
//   pnpm perf:bd-rebuild summarize <out>
//   pnpm perf:bd-rebuild compare <out> <other out>
//
// An edit inserts `let _ = std::hint::black_box(<n>u32);` into the body of one function, a new `n` per run, and the
// original bytes come back at the end, also on Ctrl-C. The file is found by its name in `crates/*/src/`, so an edit
// names the same function before and after a move (of two files of that name, the one with the function); the crate of its directory is its owner. Per edit, each scenario
// is built once untimed, then timed `--runs` times:
//   own        cargo nextest run -p <owner> --no-run   the inner loop of the owner's tests
//   cli        cargo nextest run -p bd-cli --no-run    the tests of the crate with the `Cli`
//   bin        cargo build -p bd-cli                   the `bd` binary
//   workspace  cargo nextest run --workspace --no-run  with --workspace, 3 runs at most
// A run waits until no other `rustc` runs (other checkouts build on the same machine), at most ten minutes, and is
// marked contended after that. `--clean` first times `cargo build -p bd-cli` and the test build of every bd crate after
// `cargo clean -p` of the bd crates, with the dependencies built. `--phases` runs `own` once more with `-Z time-passes`
// in a target directory of its own (a full build of the dependencies the first time).
//
// `measure` writes `machine.json`, `runs.tsv` and the `--timings` report of each first run; `summarize` prints the
// median, min and max per edit and scenario; `compare` prints both medians and their ratio. Each run keeps the wall time
// and the CPU time of cargo and the compilers it waited for (`/usr/bin/time`, user + sys): under the load of other
// sessions the wall time of the same build varies by a factor of two or more, the CPU time much less.
import { execFileSync, spawnSync } from "node:child_process"
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync, appendFileSync } from "node:fs"
import { loadavg } from "node:os"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..")

// the file and the line its function starts with: the change goes into the first line from there that opens the body
const EDITS = {
  safe_push: ["safe_push_command.rs", "pub fn safe_push("],
  clustering: ["clustering.rs", "pub fn name_clusters("],
  lease: ["lease_command.rs", "pub fn lease_status("],
  sync: ["sync_command.rs", "pub fn sync_locked("],
  check_command: ["check_command.rs", "pub fn check("],
  update_command: ["update_command.rs", "pub fn update("],
  remote: ["pull_requests.rs", "pub async fn list_pull_requests("],
  check: ["check.rs", "pub fn check_plan("],
  update: ["update_project.rs", "pub fn update_project_core("],
  lease_pool: ["lease.rs", "pub fn pool_dir("],
}
const WAIT_LIMIT_MS = 10 * 60 * 1000

const [command, ...rest] = process.argv.slice(2)
const commands = { measure, summarize, compare }
if (!Object.hasOwn(commands, command)) {
  console.error(`usage: pnpm perf:bd-rebuild ${Object.keys(commands).join("|")} … (see the header of scripts/perf/bd-rebuild.mjs)`)
  process.exit(2)
}
commands[command](rest)

/** `--name value` options, `--flag` flags and the positional arguments of `args` */
function parse(args, flags = []) {
  const options = new Map()
  const positional = []
  for (let index = 0; index < args.length; index++) {
    const arg = args[index]
    if (!arg.startsWith("--")) {
      positional.push(arg)
    } else if (flags.includes(arg)) {
      options.set(arg.slice(2), true)
    } else {
      options.set(arg.slice(2), args[++index])
    }
  }
  return { options, positional }
}

function git(...args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim()
}

function measure(args) {
  const { options } = parse(args, ["--workspace", "--clean", "--phases"])
  const out = path.resolve(options.get("out") ?? fail("--out is required"))
  const runs = Number(options.get("runs") ?? 5)
  const edits = (options.get("edits") ?? "safe_push,clustering,lease,sync").split(",")
  for (const edit of edits) {
    if (!Object.hasOwn(EDITS, edit)) {
      fail(`no edit ${edit}; known: ${Object.keys(EDITS).join(", ")}`)
    }
  }
  if (git("status", "--porcelain") !== "") {
    fail("the working tree has changes: an edit restores the file it changed, so it needs a clean one")
  }
  mkdirSync(out, { recursive: true })
  writeFileSync(path.join(out, "machine.json"), `${JSON.stringify(machine(), null, 2)}\n`)
  const tsv = path.join(out, "runs.tsv")
  if (!existsSync(tsv)) {
    writeFileSync(tsv, "edit\tscenario\trun\tseconds\tcpu\tload\tcontended\n")
  }
  const record = (edit, scenario, run, { wall, cpu }, contended) =>
    appendFileSync(tsv, `${edit}\t${scenario}\t${run}\t${wall.toFixed(2)}\t${cpu.toFixed(2)}\t${loadavg()[0].toFixed(1)}\t${contended}\n`)

  const packages = bdPackages()
  if (options.get("clean")) {
    cargo(["clean", ...packages.flatMap(name => ["-p", name])])
    for (const { scenario, build } of [
      { scenario: "clean-build", build: ["build", "-p", "bd-cli", "--timings"] },
      { scenario: "clean-tests", build: ["nextest", "run", ...packages.flatMap(name => ["-p", name]), "--no-run", "--timings"] },
    ]) {
      const contended = !waitForIdle()
      record("clean", scenario, 1, timed(build), contended)
      copyTimings(path.join(out, `${scenario}.html`))
    }
  }

  for (const edit of edits) {
    const [file, anchor] = EDITS[edit]
    const source = sourceFile(file, anchor)
    const owner = source.split("/")[1]
    const scenarios = [
      { scenario: "own", build: ["nextest", "run", "-p", owner, "--no-run"], count: runs },
      { scenario: "cli", build: ["nextest", "run", "-p", "bd-cli", "--no-run"], count: runs },
      { scenario: "bin", build: ["build", "-p", "bd-cli"], count: runs },
    ]
    if (options.get("workspace")) {
      scenarios.push({ scenario: "workspace", build: ["nextest", "run", "--workspace", "--no-run"], count: Math.min(runs, 3) })
    }
    const original = readFileSync(path.join(root, source))
    const restore = () => writeFileSync(path.join(root, source), original)
    const onSignal = () => {
      restore()
      process.exit(130)
    }
    process.once("SIGINT", onSignal)
    let change = 0
    try {
      for (const { scenario, build, count } of scenarios) {
        // owner bd-cli: `own` and `cli` are the same build
        if (scenario === "cli" && owner === "bd-cli") {
          continue
        }
        cargo(build)
        for (let run = 1; run <= count; run++) {
          writeFileSync(path.join(root, source), withChange(original.toString("utf8"), anchor, ++change, source))
          const contended = !waitForIdle()
          const timings = run === 1 ? ["--timings"] : []
          record(edit, scenario, run, timed([...build, ...timings]), contended)
          if (timings.length > 0) {
            copyTimings(path.join(out, `${edit}-${scenario}.html`))
          }
        }
      }
      if (options.get("phases")) {
        writeFileSync(path.join(root, source), withChange(original.toString("utf8"), anchor, ++change, source))
        const phases = spawnSync("cargo", ["nextest", "run", "-p", owner, "--no-run"], {
          cwd: root,
          encoding: "utf8",
          env: { ...process.env, CARGO_TARGET_DIR: path.join(root, "target/bd-rebuild-phases"), RUSTFLAGS: "-Zthreads=8 -Ztime-passes" },
        })
        writeFileSync(path.join(out, `${edit}-phases.txt`), phases.stderr)
      }
    } finally {
      restore()
      process.removeListener("SIGINT", onSignal)
    }
    // the next edit starts from the built original
    cargo(["nextest", "run", "-p", owner, "--no-run"])
  }
  summarize([out])
}

function machine() {
  const sysctl = name => execFileSync("sysctl", ["-n", name], { encoding: "utf8" }).trim()
  return {
    cpu: sysctl("machdep.cpu.brand_string"),
    cores: Number(sysctl("hw.ncpu")),
    memory_gb: Number(sysctl("hw.memsize")) / 2 ** 30,
    rustc: execFileSync("rustc", ["-Vv"], { cwd: root, encoding: "utf8" }).trim(),
    cargo: execFileSync("cargo", ["-V"], { cwd: root, encoding: "utf8" }).trim(),
    nextest: execFileSync("cargo", ["nextest", "--version"], { cwd: root, encoding: "utf8" }).split("\n")[0],
    head: git("rev-parse", "HEAD"),
    cargo_config: readFileSync(path.join(root, ".cargo/config.toml"), "utf8"),
  }
}

/** The library of the bd commands outside `crates/bd-*` (`bd-clustering` until the split of sync-core), so that
 * `--clean` cleans and builds the same code before and after the rename */
const BD_LIBRARIES = ["commit-clustering"]

/** The workspace packages under `crates/bd-*` and `BD_LIBRARIES` */
function bdPackages() {
  const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], { cwd: root, encoding: "utf8", maxBuffer: 64 << 20 }))
  return metadata.packages
    .filter(it => path.relative(root, path.dirname(it.manifest_path)).startsWith(path.join("crates", "bd-")) || BD_LIBRARIES.includes(it.name))
    .map(it => it.name)
    .sort()
}

/** The one `crates/<crate>/src/<name>` with a line starting with `anchor` */
function sourceFile(name, anchor) {
  const files = git("ls-files", `crates/*/src/${name}`)
    .split("\n")
    .filter(Boolean)
    .filter(file =>
      readFileSync(path.join(root, file), "utf8")
        .split("\n")
        .some(line => line.trimStart().startsWith(anchor)),
    )
  if (files.length !== 1) {
    fail(`expected one crates/*/src/${name} with ${anchor}, found ${files.length}`)
  }
  return files[0]
}

function withChange(text, anchor, value, source) {
  const lines = text.split("\n")
  const start = lines.findIndex(line => line.trimStart().startsWith(anchor))
  if (start === -1) {
    fail(`${source} has no line starting with ${anchor}`)
  }
  const open = lines.findIndex((line, index) => index >= start && line.trimEnd().endsWith("{"))
  lines.splice(open + 1, 0, `  let _ = std::hint::black_box(${value}u32);`)
  return lines.join("\n")
}

/** Waits until no `rustc` runs; false after `WAIT_LIMIT_MS` */
function waitForIdle() {
  const deadline = Date.now() + WAIT_LIMIT_MS
  while (spawnSync("pgrep", ["-x", "rustc"]).status === 0) {
    if (Date.now() > deadline) {
      return false
    }
    spawnSync("sleep", ["5"])
  }
  return true
}

function cargo(args) {
  const result = spawnSync("cargo", args, { cwd: root, stdio: ["ignore", "ignore", "inherit"] })
  if (result.status !== 0) {
    fail(`cargo ${args.join(" ")} exited with ${result.status}`)
  }
}

/** The wall time and the CPU time (user + sys of cargo and the processes it waited for) of `cargo args` in seconds */
function timed(args) {
  const start = performance.now()
  const result = spawnSync("/usr/bin/time", ["-p", "cargo", ...args], { cwd: root, encoding: "utf8", stdio: ["ignore", "ignore", "pipe"], maxBuffer: 64 << 20 })
  const wall = (performance.now() - start) / 1000
  if (result.status !== 0) {
    process.stderr.write(result.stderr)
    fail(`cargo ${args.join(" ")} exited with ${result.status}`)
  }
  const seconds = name => Number(new RegExp(String.raw`^${name} +([\d.]+)$`, "m").exec(result.stderr)?.[1] ?? Number.NaN)
  return { wall, cpu: seconds("user") + seconds("sys") }
}

function copyTimings(target) {
  const report = path.join(root, "target/cargo-timings/cargo-timing.html")
  if (existsSync(report)) {
    copyFileSync(report, target)
  }
}

/** edit/scenario -> the wall and CPU seconds of its uncontended runs */
function readRuns(out) {
  const runs = new Map()
  const [, ...rows] = readFileSync(path.join(out, "runs.tsv"), "utf8").trim().split("\n")
  for (const row of rows) {
    const [edit, scenario, , wall, cpu, , contended] = row.split("\t")
    if (contended === "true") {
      continue
    }
    const key = `${edit}/${scenario}`
    const values = runs.get(key) ?? { wall: [], cpu: [] }
    values.wall.push(Number(wall))
    values.cpu.push(Number(cpu))
    runs.set(key, values)
  }
  return runs
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2
}

function summarize(args) {
  const out = args[0] ?? fail("usage: summarize <out>")
  const range = values => [median(values), Math.min(...values), Math.max(...values)].map(it => it.toFixed(1))
  console.log(["edit/scenario", "runs", "wall median", "min", "max", "cpu median", "min", "max"].join("\t"))
  for (const [key, { wall, cpu }] of readRuns(out)) {
    console.log([key, wall.length, ...range(wall), ...range(cpu)].join("\t"))
  }
}

function compare(args) {
  const [before, after] = args.length === 2 ? args : fail("usage: compare <out> <other out>")
  const first = readRuns(before)
  const second = readRuns(after)
  const columns = (a, b) => [
    a ? median(a).toFixed(1) : "-",
    b ? median(b).toFixed(1) : "-",
    a && b ? (median(b) / median(a)).toFixed(2) : "-",
    a ? (Math.max(...a) - Math.min(...a)).toFixed(1) : "-",
  ]
  console.log(["edit/scenario", "wall before", "after", "ratio", "spread", "cpu before", "after", "ratio", "spread"].join("\t"))
  for (const key of new Set([...first.keys(), ...second.keys()])) {
    const a = first.get(key)
    const b = second.get(key)
    console.log([key, ...columns(a?.wall, b?.wall), ...columns(a?.cpu, b?.cpu)].join("\t"))
  }
}

function fail(message) {
  console.error(message)
  process.exit(1)
}
