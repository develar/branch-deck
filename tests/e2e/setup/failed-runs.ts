import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "fs"
import { basename, dirname, join } from "path"

/** Where a run with failed tests keeps their evidence, a directory per run; a later run overwrites none of it */
export const FAILED_RUNS = "tests/failed-runs"
/** The newest runs kept */
const KEPT = 10

/** The evidence of a failed test: its title and the output directory of its attachments (`error-context.md`, a trace) */
export interface FailedTest {
  readonly title: string
  readonly outputDir: string
}

/**
 * Keeps the evidence of the failed tests of a run in `tests/failed-runs/<start>/`: the output directory of each (its
 * `error-context.md`, its trace when the run recorded one, its screenshots), the step profile of each failed test
 * (`steps.json` of the timing reporter, the steps and where its time went) and the timing of the run; `tests/results`
 * is emptied by the next run, these stay until ten newer runs failed. Nothing for a run without a failure
 */
export function keepFailedRun(started: number, failed: readonly FailedTest[], steps: unknown, timing: unknown): string | null {
  if (failed.length === 0) {
    return null
  }
  const run = join(FAILED_RUNS, new Date(started).toISOString().replaceAll(":", "-"))
  try {
    mkdirSync(run, { recursive: true })
    for (const test of failed) {
      if (existsSync(test.outputDir)) {
        cpSync(test.outputDir, join(run, basename(test.outputDir)), { recursive: true })
      }
    }
    writeFileSync(join(run, "failed.json"), JSON.stringify(failed, null, 2))
    writeFileSync(join(run, "steps.json"), JSON.stringify(steps))
    writeFileSync(join(run, "timing.json"), JSON.stringify(timing, null, 2))
    const runs = readdirSync(FAILED_RUNS).sort()
    for (const old of runs.slice(0, Math.max(0, runs.length - KEPT))) {
      rmSync(join(FAILED_RUNS, old), { recursive: true, force: true })
    }
    return run
  } catch {
    // a disk that refuses: the run's own results are in tests/results
    return null
  }
}

/** The output directory of a test result, the directory of its attachments that have a file; null without one */
export function outputDirOf(attachments: readonly { readonly path?: string }[]): string | null {
  const path = attachments.find(attachment => attachment.path !== undefined)?.path
  return path === undefined ? null : dirname(path)
}
