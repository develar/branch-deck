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

/** Reads the status of every tool again; a read running already is joined. A failure keeps the last status */
export function refreshCliTools(): Promise<void> {
  reading ??= commands
    .cliToolsStatus()
    .then(result => {
      if (result.status === "ok") {
        cliToolsStore.setState({ statuses: result.data })
      }
    })
    .finally(() => {
      reading = null
    })
  return reading
}

function setRunning(tool: CliTool, running: RunningCliToolAction | undefined): void {
  cliToolsStore.setState(state => ({ running: { ...state.running, [tool]: running } }))
}

/** Runs the action of a row, its lines shown under it; then the toast of what it did and the status again */
export async function runCliToolAction(status: CliToolStatus): Promise<void> {
  const { tool, action } = status
  if (action === null || cliToolsStore.getState().running[tool] !== undefined) {
    return
  }
  setRunning(tool, { action, line: "" })
  const progress = new Channel<CliToolProgress>()
  progress.onmessage = ({ line }) => {
    if (cliToolsStore.getState().running[tool] !== undefined) {
      setRunning(tool, { action, line })
    }
  }
  try {
    const result = await commands.runCliToolAction(tool, action, status.latest, progress)
    if (result.status === "ok") {
      showToast({ title: result.data.title })
    } else if (result.error.message !== "canceled") {
      notifyError(`Cannot ${actionVerb(action)} ${tool}`, result.error)
    }
  } catch (error) {
    notifyError(`Cannot ${actionVerb(action)} ${tool}`, error)
  } finally {
    setRunning(tool, undefined)
  }
  await refreshCliTools()
}

/** Ends the action running for `tool`; its command answers "canceled", which toasts nothing */
export function cancelCliToolAction(tool: CliTool): void {
  void commands.cancelCliToolAction(tool)
}

function actionVerb(action: CliToolAction): string {
  switch (action) {
    case "install":
      return "install"
    case "update":
      return "update"
    case "updateSkill":
      return "update the skill of"
    case "signIn":
      return "sign in"
  }
}
