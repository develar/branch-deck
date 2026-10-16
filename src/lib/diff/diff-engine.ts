import { registerCustomTheme } from "@pierre/diffs"
import type { BaseDiffOptions } from "@pierre/diffs"
import { getOrCreateWorkerPoolSingleton } from "@pierre/diffs/worker"
import type { WorkerPoolManager } from "@pierre/diffs/worker"
import DiffsWorker from "@pierre/diffs/worker/worker.js?worker"
import { registerDiffLanguages } from "@/lib/diff/diff-languages"
import { ISLANDS_DARK_THEME, ISLANDS_LIGHT_THEME } from "@/lib/diff/islands-themes"
import { logError } from "@/lib/errors"

/*
 * The diff engine (@pierre/diffs, docs/design/diff-view.md): our syntax themes, the extensions Shiki doesn't map, and
 * one worker pool for the life of the window. Imported only by the lazy diff components, so neither Shiki nor a worker
 * is loaded before a diff is shown.
 */

export const DIFF_THEMES = { light: ISLANDS_LIGHT_THEME.name ?? "bd-islands-light", dark: ISLANDS_DARK_THEME.name ?? "bd-islands-dark" }

registerCustomTheme(DIFF_THEMES.light, () => Promise.resolve(ISLANDS_LIGHT_THEME))
registerCustomTheme(DIFF_THEMES.dark, () => Promise.resolve(ISLANDS_DARK_THEME))

registerDiffLanguages()

/** The options every diff shares; a view adds the layout (`diffStyle`), the header and the context loader */
export const BASE_DIFF_OPTIONS = {
  theme: DIFF_THEMES,
  // both themes are highlighted, so the OS switch needs no re-render; the app has no theme toggle of its own
  themeType: "system",
  preferredHighlighter: "shiki-js",
  overflow: "wrap",
  lineDiffType: "word-alt",
  diffIndicators: "bars",
} as const satisfies BaseDiffOptions

let workerPool: WorkerPoolManager | undefined
let workerErrorLogged = false

/**
 * The worker pool of the window, created on the first diff and never terminated: a provider per dialog would start
 * and stop the workers on every open. When the workers fail to start, the diffs are highlighted on the main thread.
 */
export function getDiffWorkerPool(): WorkerPoolManager {
  workerPool ??= getOrCreateWorkerPoolSingleton({
    poolOptions: {
      workerFactory: () => new DiffsWorker(),
      poolSize: Math.max(1, Math.min(3, (navigator.hardwareConcurrency || 4) >> 2)),
      onWorkerError: event => {
        if (!workerErrorLogged) {
          workerErrorLogged = true
          logError("Cannot start the diff highlighter", event instanceof ErrorEvent ? (event.error ?? event.message) : "the worker script did not load")
        }
      },
    },
    highlighterOptions: {
      theme: DIFF_THEMES,
      preferredHighlighter: "shiki-js",
      lineDiffType: "word-alt",
      langs: ["kotlin", "java", "typescript", "tsx", "rust", "json", "yaml", "markdown", "xml"],
    },
  })
  return workerPool
}
