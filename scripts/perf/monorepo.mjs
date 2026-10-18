// Measures the git processes and the wall time of `bd` on a large repository, pinned at two commits so the numbers of
// two builds compare (docs/design/git-operations.md, "Measuring on a large repository").
//
//   pnpm perf:monorepo pin --source <repo> --head <commit> --base <commit> --prefix <prefix> [--stacks] --dir <work>
//   pnpm perf:monorepo measure --dir <work> --bd <binary> --out <dir> [--runs 3] [--push <branch>] [--count]
//   pnpm perf:monorepo summarize <out> [<count out>]
//   pnpm perf:monorepo compare <out> <other out>
//   pnpm perf:monorepo reset --dir <work> [--cache-only]
//
// `pin` makes `<work>/clone`, a shared clone of `<source>` without a working tree: `master` at `--head`, its upstream
// `origin/master` at `--base`, the stack parents, issue groups and pushed marks of the source, the branch prefix, and a
// push remote `<work>/remote.git` whose branches are the clone's remote branches (`bd` never fetches it). It keeps the
// refs and the config, which `reset` restores: every ref a `bd` run made is deleted, the object cache too.
//
// `measure` runs each step `--runs` times on the state the step before left (a full reset before the first, an empty
// object cache before a cold one), `bd -C <clone> -v`, and keeps per run the exit code, the wall time and the "git
// processes" line of `bd -v`, the processes of the executor by subcommand: `runs.tsv`, the outputs as
// `<step>-<run>.json` and `.log`. `--count` runs each step once through `git-count.sh` (`BD_GIT`), which records every
// git process, also those outside the executor (the `git version` of the discovery); its times mean nothing.
// `summarize` prints the tables, `compare` checks that two builds printed the same.
import { execFileSync, spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import { appendFileSync, copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const here = path.dirname(fileURLToPath(import.meta.url))

// the refs of the source that a sync reads besides the branches
const SOURCE_REFS = ["refs/branch-deck/stack-parents", "refs/branch-deck/issue-groups", "refs/branch-deck/pushed/"]
// options of git before its subcommand that take a value, as the executor's process stats skip them
const GLOBAL_OPTIONS_WITH_VALUE = ["-C", "-c", "--git-dir", "--work-tree", "--namespace", "--super-prefix", "--config-env", "--attr-source"]

const [command, ...rest] = process.argv.slice(2)
const commands = { pin, reset, measure, summarize, compare }
if (!Object.hasOwn(commands, command)) {
  console.error(`usage: pnpm perf:monorepo ${Object.keys(commands).join("|")} … (see the header of ${path.relative(process.cwd(), fileURLToPath(import.meta.url))})`)
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
      options.set(arg, true)
    } else if (index + 1 < args.length) {
      options.set(arg, args[++index])
    } else {
      fail(`${arg} needs a value`)
    }
  }
  return { options, positional }
}

function required(options, name) {
  return options.get(name) ?? fail(`${name} is required`)
}

function fail(message) {
  console.error(message)
  process.exit(2)
}

function git(cwd, args, input) {
  return execFileSync("git", ["-C", cwd, ...args], { encoding: "utf8", input, maxBuffer: 1 << 28 }).trimEnd()
}

/** The paths of a work dir */
function work(dir) {
  const root = path.resolve(dir)
  return {
    root,
    clone: path.join(root, "clone"),
    remote: path.join(root, "remote.git"),
    refs: path.join(root, "pinned-refs.txt"),
    config: path.join(root, "pinned-config"),
  }
}

function pin(args) {
  const { options } = parse(args, ["--stacks"])
  const source = path.resolve(required(options, "--source"))
  const head = required(options, "--head")
  const base = required(options, "--base")
  const prefix = required(options, "--prefix")
  const paths = work(required(options, "--dir"))
  if (existsSync(paths.clone)) {
    fail(`${paths.clone} exists: pin into another --dir`)
  }
  mkdirSync(paths.root, { recursive: true })
  // no index and no working tree: bd reads only the object database and the refs
  execFileSync("git", ["clone", "--quiet", "--shared", "--no-checkout", source, paths.clone], { stdio: "inherit" })
  const clone = paths.clone
  git(clone, ["update-ref", "refs/heads/master", git(source, ["rev-parse", "--verify", `${head}^{commit}`])])
  git(clone, ["symbolic-ref", "HEAD", "refs/heads/master"])
  git(clone, ["update-ref", "refs/remotes/origin/master", git(source, ["rev-parse", "--verify", `${base}^{commit}`])])
  git(clone, ["config", "branch.master.remote", "origin"])
  git(clone, ["config", "branch.master.merge", "refs/heads/master"])
  const sourceRefs = git(source, ["for-each-ref", "--format=%(refname)", ...SOURCE_REFS])
    .split("\n")
    .filter(Boolean)
  if (sourceRefs.length > 0) {
    git(clone, ["fetch", "--quiet", source, ...sourceRefs.map(ref => `${ref}:${ref}`)])
  }
  git(clone, ["config", "branchdeck.branchprefix", prefix])
  if (options.get("--stacks")) {
    git(clone, ["config", "branchdeck.stacks", "true"])
  }

  // the push remote: the objects of the source through alternates, the branches the clone has as remote branches
  execFileSync("git", ["init", "--quiet", "--bare", paths.remote])
  const objects = path.join(git(source, ["rev-parse", "--path-format=absolute", "--git-common-dir"]), "objects")
  writeFileSync(path.join(paths.remote, "objects", "info", "alternates"), `${objects}\n`)
  const branches = git(clone, ["for-each-ref", "--format=create refs/heads/%(refname:lstrip=3) %(objectname)", "refs/remotes/origin/"])
    .split("\n")
    .filter(line => line && !line.startsWith("create refs/heads/HEAD "))
  git(paths.remote, ["update-ref", "--stdin"], branches.map(line => `${line}\n`).join(""))
  git(paths.remote, ["symbolic-ref", "HEAD", "refs/heads/master"])
  git(clone, ["config", "remote.origin.pushurl", paths.remote])

  writeFileSync(paths.refs, `${listRefs(clone).join("\n")}\n`)
  copyFileSync(path.join(clone, ".git", "config"), paths.config)
  console.log(`pinned ${clone}: ${listRefs(clone).length} refs, ${branches.length} remote branches`)
}

/** `<id> <ref>` of every ref of `repo` that is no symbolic ref */
function listRefs(repo) {
  return git(repo, ["for-each-ref", "--format=%(if)%(symref)%(then)%(else)%(objectname) %(refname)%(end)"]).split("\n").filter(Boolean)
}

function reset(args) {
  const { options } = parse(args, ["--cache-only"])
  restore(work(required(options, "--dir")), options.get("--cache-only") === true)
}

function restore(paths, cacheOnly) {
  const state = path.join(paths.clone, ".git", "branch-deck")
  if (cacheOnly) {
    rmSync(path.join(state, "cache"), { recursive: true, force: true })
    return
  }
  git(paths.clone, ["symbolic-ref", "HEAD", "refs/heads/master"])
  const pinned = readFileSync(paths.refs, "utf8").split("\n").filter(Boolean)
  const names = new Set(pinned.map(line => line.slice(line.indexOf(" ") + 1)))
  const deletes = listRefs(paths.clone)
    .map(line => line.slice(line.indexOf(" ") + 1))
    .filter(ref => !names.has(ref))
    .map(ref => `delete ${ref}\n`)
  const updates = pinned.map(line => {
    const [id, ref] = [line.slice(0, line.indexOf(" ")), line.slice(line.indexOf(" ") + 1)]
    return `update ${ref} ${id}\n`
  })
  git(paths.clone, ["update-ref", "--stdin"], [...deletes, ...updates].join(""))
  copyFileSync(paths.config, path.join(paths.clone, ".git", "config"))
  rmSync(state, { recursive: true, force: true })
  rmSync(`${state}.lock`, { force: true })
}

/** The steps of `measure`, in order: what runs before each run (a full reset, an empty object cache, nothing) */
function steps(pushBranch) {
  return [
    { label: "sync-full", before: "full", args: ["sync"] },
    { label: "sync-unchanged", before: "none", args: ["sync"] },
    { label: "status", before: "none", args: ["status"] },
    { label: "commits-cold", before: "cache", args: ["commits"] },
    { label: "commits-warm", before: "none", args: ["commits"] },
    { label: "check-cold", before: "cache", args: ["check"] },
    { label: "check-warm", before: "none", args: ["check"] },
    ...(pushBranch ? [{ label: "push-dry", before: "none", args: ["push", pushBranch, "--dry-run"] }] : []),
    { label: "push-all-dry", before: "none", args: ["push", "--all", "--yes", "--force", "--dry-run"] },
  ]
}

function measure(args) {
  const { options } = parse(args, ["--count"])
  const paths = work(required(options, "--dir"))
  const bd = path.resolve(required(options, "--bd"))
  const out = path.resolve(required(options, "--out"))
  const count = options.get("--count") === true
  const runs = count ? 1 : Number(options.get("--runs") ?? 3)
  mkdirSync(out, { recursive: true })
  const binary = readFileSync(bd)
  writeFileSync(path.join(out, "binary.txt"), `${bd}\n${createHash("sha256").update(binary).digest("hex")}\n${execFileSync(bd, ["--version"], { encoding: "utf8" })}`)
  writeFileSync(path.join(out, "runs.tsv"), "")
  writeFileSync(path.join(out, "load.txt"), "")
  const realGit = count ? execFileSync("sh", ["-c", "command -v git"], { encoding: "utf8" }).trim() : null

  for (const step of steps(options.get("--push"))) {
    appendFileSync(path.join(out, "load.txt"), `${step.label} ${execFileSync("uptime", { encoding: "utf8" })}`)
    for (let run = 1; run <= runs; run++) {
      if (step.before !== "none") {
        restore(paths, step.before === "cache")
      }
      let env = process.env
      if (count) {
        const dir = path.join(out, `${step.label}-${run}.git`)
        rmSync(dir, { recursive: true, force: true })
        mkdirSync(dir)
        env = { ...process.env, BD_GIT: path.join(here, "git-count.sh"), GIT_COUNT_DIR: dir, GIT_COUNT_REAL: realGit }
      }
      const start = performance.now()
      const result = spawnSync(bd, ["-C", paths.clone, "-v", ...step.args], { env, stdio: ["ignore", "pipe", "pipe"], maxBuffer: 1 << 28 })
      const wall = (performance.now() - start) / 1000
      writeFileSync(path.join(out, `${step.label}-${run}.json`), result.stdout)
      writeFileSync(path.join(out, `${step.label}-${run}.log`), result.stderr)
      const stats = result.stderr
        .toString()
        .split("\n")
        .filter(line => line.includes("git processes "))
        .map(line => line.slice(line.indexOf("git processes ") + "git processes ".length))
        .join("|")
      appendFileSync(path.join(out, "runs.tsv"), `${step.label}\t${run}\t${result.status ?? result.signal}\t${wall.toFixed(3)}\t${stats}\n`)
      console.log(`${step.label} ${run}: exit ${result.status ?? result.signal}, ${wall.toFixed(2)} s`)
    }
  }
  appendFileSync(path.join(out, "load.txt"), `end ${execFileSync("uptime", { encoding: "utf8" })}`)
}

/** The runs of `runs.tsv` by step: `{code, wall, total, byCommand, lines}` */
function readRuns(out) {
  const runs = new Map()
  for (const line of readFileSync(path.join(out, "runs.tsv"), "utf8").split("\n").filter(Boolean)) {
    const [label, , code, wall, stats = ""] = line.split("\t")
    if (!runs.has(label)) {
      runs.set(label, [])
    }
    runs.get(label).push({ code, wall: Number(wall), ...parseStats(stats) })
  }
  return runs
}

/** `operation=bd processes=11 … commands=log 1×210ms, rev-parse 3×26ms|…`: the total and the count of each subcommand */
function parseStats(stats) {
  const lines = stats.split("|").filter(Boolean)
  const byCommand = new Map()
  let total = 0
  for (const line of lines) {
    total += Number(/processes=(\d+)/.exec(line)?.[1] ?? 0)
    const commands = line.includes("commands=") ? line.slice(line.indexOf("commands=") + "commands=".length) : ""
    for (const [, name, n] of commands.matchAll(/([\w-]+) (\d+)×\d+ms/g)) {
      byCommand.set(name, (byCommand.get(name) ?? 0) + Number(n))
    }
  }
  return { total, byCommand, lines: lines.length }
}

/** Most processes first */
function format(byCommand) {
  return [...byCommand]
    .filter(([, n]) => n > 0)
    .sort(([a, x], [b, y]) => y - x || a.localeCompare(b))
    .map(([name, n]) => `${name} ${n}`)
    .join(", ")
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2
}

function summarize(args) {
  const { positional } = parse(args)
  const [timed, counted] = positional
  if (!timed) {
    fail("summarize <out> [<count out>]")
  }
  const runs = readRuns(timed)
  console.log("| step | exit | wall min / median s | processes (bd -v) | by subcommand (bd -v) |")
  console.log("|---|---|---|---|---|")
  for (const [label, rs] of runs) {
    const codes = [...new Set(rs.map(r => r.code))].sort((a, b) => a.localeCompare(b)).join(",")
    const walls = rs.map(r => r.wall)
    const totals = rs.map(r => r.total)
    const details = rs.map(r => format(r.byCommand))
    const total = new Set(totals).size === 1 ? `${totals[0]}` : totals.join("/")
    const detail = new Set(details).size === 1 ? details[0] : details.join(" / ")
    const extra = rs.every(r => r.lines === 1) ? "" : ` (${rs[0].lines} stats lines)`
    console.log(`| ${label} | ${codes} | ${Math.min(...walls).toFixed(2)} / ${median(walls).toFixed(2)} | ${total}${extra} | ${detail} |`)
  }
  if (!counted) {
    return
  }
  console.log()
  console.log("| step | all git processes (git-count.sh) | bd -v of that run | outside the stats |")
  console.log("|---|---|---|---|")
  for (const [label, [run]] of readRuns(counted)) {
    const dir = path.join(counted, `${label}-1.git`)
    if (!existsSync(dir)) {
      continue
    }
    const seen = new Map()
    for (const name of readdirSync(dir)) {
      const subcommand = subcommandOf(readFileSync(path.join(dir, name), "utf8").split("\x1f").slice(0, -1))
      seen.set(subcommand, (seen.get(subcommand) ?? 0) + 1)
    }
    const outside = new Map([...seen].map(([name, n]) => [name, n - (run.byCommand.get(name) ?? 0)]))
    const all = [...seen.values()].reduce((sum, n) => sum + n, 0)
    console.log(`| ${label} | ${all} | ${run.total} | ${format(outside)} |`)
  }
}

/** The git subcommand of the arguments of a process, `git` for none */
function subcommandOf(args) {
  for (let index = 0; index < args.length; index++) {
    if (GLOBAL_OPTIONS_WITH_VALUE.includes(args[index])) {
      index++
    } else if (!args[index].startsWith("-")) {
      return args[index]
    }
  }
  return "git"
}

/** The outputs of the first run of each step of two measurements are the same, but for the path of the binary */
function compare(args) {
  const { positional } = parse(args)
  const [a, b] = positional
  if (!a || !b) {
    fail("compare <out> <other out>")
  }
  const output = (out, label) => {
    const bd = readFileSync(path.join(out, "binary.txt"), "utf8").split("\n")[0]
    return readFileSync(path.join(out, `${label}-1.json`), "utf8").replaceAll(bd, "<bd>")
  }
  let differ = 0
  for (const label of readRuns(a).keys()) {
    if (!existsSync(path.join(b, `${label}-1.json`))) {
      continue
    }
    const same = output(a, label) === output(b, label)
    differ += same ? 0 : 1
    console.log(`${same ? "same  " : "DIFFER"} ${label}`)
  }
  process.exit(differ === 0 ? 0 : 1)
}
