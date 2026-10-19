import { HighlightClient, inProcessPort, workerPort } from "@/lib/diff/highlight/client"
import { createGrammarSource } from "@/lib/diff/highlight/grammars"
import HighlightWorker from "@/lib/diff/highlight/highlight.worker.ts?worker"
import { logError } from "@/lib/errors"

/*
 * The highlighting of the window (docs/design/diff-view.md, "Syntax highlighting"): created with the first diff and
 * never stopped. The main window has two workers, a review window one. A worker that does not start is replaced by
 * the engine on the main thread, in idle slices of 4 ms.
 */

let client: HighlightClient | undefined
let failureLogged = false

function idleTask(): Promise<void> {
  return new Promise(resolve => {
    if (typeof requestIdleCallback === "function") {
      requestIdleCallback(() => resolve(), { timeout: 50 })
    } else {
      setTimeout(resolve, 0)
    }
  })
}

export function getHighlightClient(): HighlightClient {
  if (client !== undefined) {
    return client
  }
  const count = window.__SUB_WINDOW_OPEN__ === undefined ? 2 : 1
  let mainThread: ReturnType<typeof inProcessPort> | undefined
  const workers = Array.from({ length: count }, () => new HighlightWorker())
  const created = new HighlightClient(workers.map(workerPort), undefined, (blobId, reason) => {
    if (!failureLogged) {
      failureLogged = true
      logError("Cannot highlight a diff", `${blobId}: ${reason}`)
    }
  })
  workers.forEach((worker, index) => {
    worker.addEventListener("error", event => {
      logError("Cannot start the diff highlighter", event.error ?? event.message)
      mainThread ??= inProcessPort(createGrammarSource(), { sliceMs: 4, yieldTask: idleTask })
      created.replacePort(index, mainThread)
    })
  })
  client = created
  return created
}
