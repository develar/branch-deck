// Where the time of slow E2E tests went, from the step profile the timing reporter writes (`tests/results/steps.json`,
// `tests/e2e/setup/timing-reporter.ts`): a test that passes under load near its 30 s timeout shows its slow steps
// and the gaps between them, and the kinds of steps of several tests sum up a class (docs/design/e2e-testing.md,
// "A test of many starts").
// Run: `node scripts/e2e-steps.mjs [tests/results/steps.json] <part of the title> [--kinds] [--min <ms>]`: the top-level
// steps of the first matching test at least `min` ms long (300) or after a gap of 150 ms; with `--kinds` the time per
// kind of step over every matching test.
import { readFileSync } from "node:fs"

/** The kind of a step: its title without what it acts on ("pw:api: Click", `expect: Expect "toHaveCount"`) */
export function stepKind(title) {
  const quoted = /"([^"]*)"/.exec(title)
  const matcher = quoted !== null && /^(poll |not )?to[A-Z]/.test(quoted[1])
  return (matcher ? title.slice(0, quoted.index + quoted[0].length) : title.replace(/ ".*$/, "")).replace(/ \S+\.(spec|setup)\.ts.*$/, "")
}

/** The time and count of each kind of the top-level steps of `profiles`, the costliest first */
export function kindsOf(profiles) {
  const kinds = new Map()
  for (const profile of profiles) {
    for (const [, ms, depth, title] of profile.steps) {
      if (depth === 0) {
        const kind = kinds.get(stepKind(title)) ?? { kind: stepKind(title), ms: 0, count: 0 }
        kind.ms += ms
        kind.count++
        kinds.set(kind.kind, kind)
      }
    }
  }
  return [...kinds.values()].sort((a, b) => b.ms - a.ms)
}

/** The top-level steps of `profile` at least `min` ms long or after a gap of 150 ms, each with the gap before it */
export function slowSteps(profile, min) {
  const steps = []
  let end = 0
  for (const [at, ms, depth, title, where] of profile.steps) {
    if (depth !== 0) {
      continue
    }
    const gap = at - end
    if (ms >= min || gap >= 150) {
      steps.push({ at, ms, gap: Math.max(gap, 0), title, where })
    }
    end = Math.max(end, at + ms)
  }
  return steps
}

function main(args) {
  const file = args[0]?.endsWith(".json") ? args.shift() : "tests/results/steps.json"
  const kinds = args.includes("--kinds")
  const minAt = args.indexOf("--min")
  const min = minAt === -1 ? 300 : Number(args[minAt + 1])
  const needle = args.find((arg, index) => !arg.startsWith("--") && args[index - 1] !== "--min") ?? ""
  const profiles = JSON.parse(readFileSync(file, "utf8")).filter(profile => profile.title.includes(needle))
  const s = ms => `${(ms / 1000).toFixed(2)} s`
  if (profiles.length === 0) {
    console.log(`no test matches "${needle}"`)
  } else if (kinds) {
    console.log(`${profiles.length} tests, ${s(profiles.reduce((sum, profile) => sum + profile.ms, 0))}`)
    for (const kind of kindsOf(profiles).slice(0, 25)) {
      console.log(`${s(kind.ms).padStart(10)} ${String(kind.count).padStart(6)} ${String(Math.round(kind.ms / kind.count)).padStart(6)} ms  ${kind.kind}`)
    }
  } else {
    const [profile] = profiles
    console.log(`${s(profile.ms)} ${profile.status} ${profile.title}`)
    for (const step of slowSteps(profile, min)) {
      console.log(`${s(step.at).padStart(8)} ${step.gap >= 150 ? `gap ${s(step.gap)}` : "".padEnd(10)} +${s(step.ms).padStart(7)} ${step.title} ${step.where}`)
    }
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2))
}
