// Replays the heavy runs of a host of brd-host under rules of the admission, so a change of `host_turn::admits` is
// judged by the runs it would have admitted (docs/design/host-runs.md, "Heavy runs").
//
//   pnpm perf:host-admission trace <history.jsonl>... > trace.json   an anonymized trace of the history of a host
//   pnpm perf:host-admission replay [trace.json]                      the waits of each rule (default: the trace of the tests)
//   pnpm perf:host-admission chain <history.jsonl>...                 how many runs without a measure each link of the chain covers
//
// The model: each heavy run arrives at its start with its CPU work and the rate of its command alone (its mean
// cores), and the running heavy runs and the runs that are not heavy share the cores by processor sharing. The rule
// "today" is the admission of brd-host 1.27: the head of the queue alone, a run without a measure takes the whole host
// and starts on a free turn only on a quiet host or after 15 min, a holder holds its measure for 10 min and then about
// its use of the last 5 min, a short run joins ahead, and a run waits at most 2 h. A run that never ran its command
// leaves the queue when it really left, unless a rule admits it before; then it runs the median work of its key.
// The hours are a model: the rules are compared by their ranks.
import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

export const CORES = 20
const GRACE = 600
const LOAD_BOUND = 900
const TIMEOUT = 7200
const SHORT_WALL = 120
const PASS_BUDGET = 4
const STEP = 30
const quiet = Math.max(1, CORES / 5)

const phase = (line, name) => (line.phases ?? []).filter(p => p.name === name).reduce((sum, p) => sum + p.ms, 0)
const hash = text => createHash("sha256").update(text).digest("hex").slice(0, 12)

/** The words of a command line with commit ids and long numbers replaced */
export function normalized(command) {
  return command
    .split(/\s+/)
    .map(word => word.replace(/(^|[=:/@])(?=[0-9a-f]*[0-9])(?=[0-9a-f]*[a-f])[0-9a-f]{7,40}(?=$|[^0-9a-z])/gi, "$1<rev>").replace(/\d{5,}/g, "<n>"))
    .join(" ")
}

/** The program of a command line: its first word and subcommand, the first command of a script of `sh -c` */
export function program(command) {
  let words = command.split(/\s+/).filter(Boolean)
  const shell = words[0]?.replace(/^.*\//, "")
  if (["sh", "bash", "zsh"].includes(shell)) {
    const c = words.findIndex(word => /^-[a-z]*c$/.test(word))
    if (c >= 0) {
      words = words.slice(c + 1).map(word => word.replace(/^['"]/, ""))
    }
  }
  words = words.filter(word => !/^[A-Za-z_][A-Za-z0-9_]*=/.test(word) && !["env", "nice", "time", "exec", "nohup"].includes(word))
  const [first, second] = words
  return [first?.replace(/[;'"]+$/, ""), second && !second.startsWith("-") && !/[$'";|&]/.test(second) ? second : ""].join(" ").trim()
}

/** The anonymized trace of history lines: the heavy runs and the background of the runs that are not heavy */
export function trace(lines) {
  const start = Math.min(...lines.map(line => line.started_ms))
  const jobs = []
  const light = []
  for (const line of lines) {
    const command = phase(line, "command")
    const cores = line.cores
    const arrive = (line.started_ms - start) / 1000
    if (!line.heavy) {
      if (cores && command > 0) {
        const at = arrive + (line.phases ?? []).filter(p => p.name !== "command").reduce((sum, p) => sum + p.ms, 0) / 1000
        light.push([Math.round(at), Math.round(at + command / 1000), cores.mean])
      }
      continue
    }
    const ran = Boolean(cores && command > 0)
    const queue = line.queue ?? {}
    jobs.push({
      arrive: Math.round(arrive),
      ran,
      wait: Math.round(phase(line, "heavy") / 1000),
      cpu: ran ? Math.round(cores.cpu_ms / 1000) : 0,
      rate: ran ? Math.max(cores.mean, 0.2) : 0,
      wall: ran ? Math.round(command / 1000) : 0,
      level: ran ? Math.max(cores.level ?? 0, cores.mean) : 0,
      exclusive: Boolean(line.exclusive),
      passed: line.exit === 0,
      // the measure of the run at its join, as the queue saw it; null without one
      seen: queue.need === "cores" ? queue.expected : queue.need === "unmeasured" || queue.need === "exclusive" ? null : undefined,
      keys: [hash(line.repo + line.command_key), hash(line.repo + normalized(line.command)), hash(line.repo + program(line.command))],
    })
  }
  jobs.sort((a, b) => a.arrive - b.arrive)
  return { cores: CORES, jobs, light }
}

/** The measures of each run by the runs of its keys that ended before it: `[exact, normalized, program]` */
function priors(jobs) {
  const ended = jobs
    .filter(job => job.ran)
    .map(job => ({ job, end: job.arrive + job.wait + job.wall }))
    .sort((a, b) => a.end - b.end)
  const p80 = values => (values.length ? [...values].sort((a, b) => a - b)[Math.floor(0.8 * (values.length - 1))] : null)
  for (const job of jobs) {
    const before = ended.filter(e => e.end < job.arrive)
    job.measures = [0, 1, 2].map(k =>
      p80(
        before
          .filter(e => e.job.keys[k] === job.keys[k])
          .slice(-5)
          .map(e => e.job.level),
      ),
    )
    const walls = before
      .filter(e => e.job.keys[0] === job.keys[0] && e.job.passed && e.job.wall >= 10)
      .slice(-5)
      .map(e => e.job.wall)
    job.shortWall = walls.length >= 3 ? p80(walls) : null
  }
  const median = new Map()
  for (const k of new Set(jobs.map(job => job.keys[0]))) {
    const ran = jobs.filter(job => job.ran && job.keys[0] === k).sort((a, b) => a.cpu - b.cpu)
    if (ran.length) {
      median.set(k, ran[Math.floor(ran.length / 2)])
    }
  }
  for (const job of jobs) {
    if (!job.ran) {
      const kin = median.get(job.keys[0])
      Object.assign(job, kin ? { cpu: kin.cpu, rate: kin.rate, wall: kin.wall } : { cpu: 360 * 6, rate: 6, wall: 360 })
    }
  }
}

/**
 * The rules of an admission: `chain` the links of the measures (1 the exact key of today, 3 with the normalized key and
 * the program), `assumed` the cores of a run without a measure (null: the whole host, as today), `factor` the
 * overcommit, `usage` whether a run past the cores needs room by the use of the host too
 */
export const RULES = {
  today: { chain: 1, assumed: null, factor: 1 },
  "stage 1: a default of 10": { chain: 1, assumed: 10, factor: 1 },
  "stage 1 + 2: the chain": { chain: 3, assumed: 10, factor: 1 },
  "stage 1 + 2 + 4 at 1.2": { chain: 3, assumed: 10, factor: 1.2, usage: true },
}

export function replay(data, rule) {
  const jobs = data.jobs.map(job => ({ ...job }))
  priors(jobs)
  const measured = job => {
    if (job.seen !== undefined && rule.chain === 1) {
      return job.seen
    }
    return job.measures.slice(0, rule.chain).find(m => m != null) ?? job.seen ?? null
  }
  const background = t => data.light.reduce((sum, [s, e, m]) => (s <= t && t < e ? sum + m : sum), 0)
  const queue = []
  const running = new Map()
  const out = new Map()
  const window = []
  let t = jobs[0].arrive
  let next = 0
  const reserve = job => {
    if (job.exclusive) {
      return { whole: true, exclusive: true }
    }
    const m = measured(job)
    if (m != null && m < CORES) {
      return { cores: m }
    }
    if (m == null && rule.assumed != null) {
      return { cores: rule.assumed, assumed: true }
    }
    return { whole: true }
  }
  const held = job => {
    const r = job.reserve
    if (r.whole && t - job.start < GRACE) {
      return CORES
    }
    if (r.assumed) {
      return Math.max(r.cores, job.most)
    }
    if (t - job.start < GRACE) {
      return Math.max(r.cores, job.live)
    }
    return Math.max(1.3 * job.rate * job.slow, job.live)
  }
  const admits = job => {
    const r = job.reserve
    const outside = background(t)
    const holders = [...running.values()]
    const waitedOut = t - job.arrive >= LOAD_BOUND
    if (r.exclusive) {
      return holders.length === 0 && outside <= quiet
    }
    if (holders.length === 0) {
      return waitedOut || outside <= quiet || (!r.whole && outside + r.cores <= CORES)
    }
    if (r.whole || holders.some(h => h.reserve.whole && t - h.start < GRACE)) {
      return false
    }
    const sum = holders.reduce((s, h) => s + held(h), 0) + outside + r.cores
    if (sum <= CORES) {
      return true
    }
    if (rule.factor <= 1 || r.assumed || holders.some(h => h.reserve.assumed)) {
      return false
    }
    const busy = Math.max(0, ...window.filter(([at]) => at > t - 300).map(([, b]) => b))
    return sum <= CORES * rule.factor && (!rule.usage || busy + r.cores <= CORES)
  }
  const place = job => {
    let at = queue.length
    if (job.shortWall != null && job.shortWall <= SHORT_WALL && !job.reserve.whole) {
      while (at > 0 && queue[at - 1].shortWall == null && queue[at - 1].passes < PASS_BUDGET) {
        at--
      }
      for (let i = at; i < queue.length; i++) {
        queue[i].passes++
      }
    }
    queue.splice(at, 0, job)
  }
  while (next < jobs.length || queue.length || running.size) {
    while (queue.length && admits(queue[0])) {
      const job = queue.shift()
      Object.assign(job, { start: t, left: job.cpu, most: 0, live: 0, slow: 1 })
      running.set(job, job)
      out.set(job, { wait: t - job.arrive, ran: true })
    }
    const outside = background(t)
    const demand = [...running.values()].reduce((s, j) => s + j.rate, 0)
    const factor = demand + outside > CORES ? CORES / (demand + outside) : 1
    window.push([t, demand * factor + Math.min(outside, CORES)])
    if (window.length > 40) {
      window.shift()
    }
    for (const job of running.values()) {
      job.live = job.rate * factor
      job.most = Math.max(job.most, job.live)
      job.slow = factor
    }
    const ends = [...running.values()].map(j => j.left / (j.rate * factor))
    const dt = Math.max(1, Math.min(STEP, ...ends, next < jobs.length ? jobs[next].arrive - t : STEP))
    for (const job of running.values()) {
      job.left -= job.rate * factor * dt
    }
    t += dt
    for (const job of running.values()) {
      if (job.left <= 1e-6) {
        running.delete(job)
        out.get(job).stretch = (t - job.start) / Math.max(job.wall, 1)
      }
    }
    for (let i = queue.length - 1; i >= 0; i--) {
      const job = queue[i]
      const gone = !job.ran && t >= job.arrive + job.wait
      if (gone || t - job.arrive >= TIMEOUT) {
        queue.splice(i, 1)
        out.set(job, { wait: t - job.arrive, ran: false })
      }
    }
    while (next < jobs.length && jobs[next].arrive <= t) {
      const job = jobs[next++]
      job.reserve = reserve(job)
      job.passes = 0
      place(job)
    }
  }
  const results = [...out.values()]
  const waits = results.map(r => r.wait).sort((a, b) => a - b)
  const stretch = results
    .filter(r => r.stretch)
    .map(r => r.stretch)
    .sort((a, b) => a - b)
  const q = (values, p) => values[Math.floor(p * (values.length - 1))] ?? 0
  return {
    waitHours: Math.round(waits.reduce((a, b) => a + b, 0) / 3600),
    p50Minutes: Math.round(q(waits, 0.5) / 60),
    p90Minutes: Math.round(q(waits, 0.9) / 60),
    neverRan: results.filter(r => !r.ran).length,
    stretchP90: Math.round(q(stretch, 0.9) * 100) / 100,
  }
}

/** The real waits of the trace, as the rule of today was measured */
export function real(data) {
  const waits = data.jobs.map(job => job.wait).sort((a, b) => a - b)
  const q = p => waits[Math.floor(p * (waits.length - 1))]
  return {
    waitHours: Math.round(waits.reduce((a, b) => a + b, 0) / 3600),
    p50Minutes: Math.round(q(0.5) / 60),
    p90Minutes: Math.round(q(0.9) / 60),
    neverRan: data.jobs.filter(job => !job.ran).length,
  }
}

function chain(lines) {
  const data = trace(lines)
  const jobs = data.jobs.map(job => ({ ...job }))
  priors(jobs)
  const unmeasured = jobs.filter(job => !job.exclusive && job.measures[0] == null && job.seen == null)
  const byNorm = unmeasured.filter(job => job.measures[1] != null).length
  const byProgram = unmeasured.filter(job => job.measures[1] == null && job.measures[2] != null).length
  console.log(`${jobs.length} heavy runs, ${unmeasured.length} without a measure of their key`)
  console.log(`  the normalized key gives ${byNorm} a measure (${Math.round((100 * byNorm) / unmeasured.length)} %)`)
  console.log(`  the program gives ${byProgram} more (${Math.round((100 * byProgram) / unmeasured.length)} %)`)
}

const here = dirname(fileURLToPath(import.meta.url))
export const TEST_TRACE = join(here, "../../tests/perf/host-admission/trace.json")
const readLines = files =>
  files.flatMap(file =>
    readFileSync(file, "utf8")
      .split("\n")
      .filter(Boolean)
      .map(line => JSON.parse(line)),
  )

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2)
  if (command === "trace") {
    process.stdout.write(JSON.stringify(trace(readLines(args))) + "\n")
  } else if (command === "chain") {
    chain(readLines(args))
  } else if (command === "replay") {
    const data = JSON.parse(readFileSync(args[0] ?? TEST_TRACE, "utf8"))
    console.log("real".padEnd(28), real(data))
    for (const [name, rule] of Object.entries(RULES)) {
      console.log(name.padEnd(28), replay(data, rule))
    }
  } else {
    console.error("usage: host-admission trace <history.jsonl>... | replay [trace.json] | chain <history.jsonl>...")
    process.exit(2)
  }
}
