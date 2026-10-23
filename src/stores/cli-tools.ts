import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { CliTool, CliToolAction, CliToolProgress, CliToolStatus } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showToast } from "@/lib/toast-message"

/*
 * The `teamcity` and `youtrack` CLIs of Settings → Tools (spec/repository/cli-tools.spec.md): the last status Rust
 * read, and the action running for each tool with the last line of its output.
 */

export interface RunningCliToolAction {
  readonly action: CliToolAction
  /** The last line of its output; empty before the first */
  readonly line: string
  /** The authorize page a sign-in opened in the browser */
  readonly url: string | null
}

interface CliToolsState {
  /** null until the first read answers; the last answer stays while the next one runs */
  readonly statuses: readonly CliToolStatus[] | null
  readonly running: Partial<Record<CliTool, RunningCliToolAction>>
}

const cliToolsStore = createStore<CliToolsState>(() => ({ statuses: null, running: {} }))

export function useCliToolStatuses(): readonly CliToolStatus[] | null {
  return useStore(cliToolsStore, state => state.statuses)
}

export function useRunningCliToolAction(tool: CliTool): RunningCliToolAction | undefined {
  return useStore(cliToolsStore, state => state.running[tool])
}

let reading: Promise<void> | null = null
let readAgain = false

/**
 * Reads the status of every tool again. A read asked while one runs is run once more after it: the one running may
 * have started before what changed (an action just ended, the user installed a tool in a terminal). A failure keeps the
 * last status
 */
export function refreshCliTools(): Promise<void> {
  if (reading !== null) {
    readAgain = true
    return reading
  }
  reading = commands
    .cliToolsStatus()
    .then(result => {
      if (result.status === "ok") {
        cliToolsStore.setState({ statuses: result.data })
      }
    })
    .finally(() => {
      reading = null
      if (readAgain) {
        readAgain = false
        void refreshCliTools()
      }
    })
  return reading
}

function setRunning(tool: CliTool, running: RunningCliToolAction | undefined): void {
  cliToolsStore.setState(state => ({ running: { ...state.running, [tool]: running } }))
}

/**
 * Runs `action` of `tool`, its lines shown under its row; then the toast of what it did and the status again, which
 * tells every window's Setup dot (`setup-changed`). `latest` names the release an update wants, `server` the server a
 * sign-in is to
 */
export async function runCliToolAction(tool: CliTool, action: CliToolAction, latest: string | null, server: string | null = null): Promise<void> {
  if (cliToolsStore.getState().running[tool] !== undefined) {
    return
  }
  setRunning(tool, { action, line: "", url: null })
  const progress = new Channel<CliToolProgress>()
  progress.onmessage = ({ line, url }) => {
    const running = cliToolsStore.getState().running[tool]
    if (running !== undefined) {
      setRunning(tool, { action, line, url: url ?? running.url })
    }
  }
  try {
    const result = await commands.runCliToolAction(tool, action, latest, server, progress)
    if (result.status === "ok") {
      showToast({ title: result.data.title })
    } else if (result.error.message !== "canceled") {
      notifyError(failedTitle(action, tool), result.error)
    }
  } catch (error) {
    notifyError(failedTitle(action, tool), error)
  } finally {
    setRunning(tool, undefined)
  }
  await refreshCliTools()
}

/** Ends the action running for `tool`; its command answers "canceled", which toasts nothing */
export function cancelCliToolAction(tool: CliTool): void {
  void commands.cancelCliToolAction(tool)
}

/** "Cannot install youtrack", "Cannot sign teamcity in" */
function failedTitle(action: CliToolAction, tool: CliTool): string {
  switch (action) {
    case "install":
      return `Cannot install ${tool}`
    case "update":
      return `Cannot update ${tool}`
    case "updateSkill":
      return `Cannot update the skill of ${tool}`
    case "signIn":
      return `Cannot sign ${tool} in`
  }
}
