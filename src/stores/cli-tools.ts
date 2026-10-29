import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { CliTool, CliToolAction, CliToolProgress, CliToolStatus, SignInEvent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorCode, notifyError } from "@/lib/errors"
import { cancelSignIn, invokeSignIn, openSignInPage, signedInTitle } from "@/lib/sign-in-run"
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
  /** Why the last read failed; null once one answers. The last statuses stay */
  readonly readError: string | null
  readonly running: Partial<Record<CliTool, RunningCliToolAction>>
  /** "Also install the agent skill" of a missing CLI: on by default, kept while the app runs, not a setting */
  readonly installWithSkill: Record<CliTool, boolean>
}

const cliToolsStore = createStore<CliToolsState>(() => ({ statuses: null, readError: null, running: {}, installWithSkill: { teamcity: true, youtrack: true } }))

export function useInstallWithSkill(tool: CliTool): boolean {
  return useStore(cliToolsStore, state => state.installWithSkill[tool])
}

export function setInstallWithSkill(tool: CliTool, value: boolean): void {
  cliToolsStore.setState(state => ({ installWithSkill: { ...state.installWithSkill, [tool]: value } }))
}

export function useCliToolStatuses(): readonly CliToolStatus[] | null {
  return useStore(cliToolsStore, state => state.statuses)
}

export function useCliToolsReadError(): string | null {
  return useStore(cliToolsStore, state => state.readError)
}

export function useRunningCliToolAction(tool: CliTool): RunningCliToolAction | undefined {
  return useStore(cliToolsStore, state => state.running[tool])
}

let reading: Promise<void> | null = null
let readAgain = false

/**
 * Reads the status of every tool again. A read asked while one runs is run once more after it: the one running may
 * have started before what changed (an action just ended, the user installed a tool in a terminal). A failure keeps the
 * last status and says why (`readError`)
 */
export function refreshCliTools(): Promise<void> {
  if (reading !== null) {
    readAgain = true
    return reading
  }
  reading = commands
    .cliToolsStatus()
    .then(result => {
      cliToolsStore.setState(result.status === "ok" ? { statuses: result.data, readError: null } : { readError: result.error.message })
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

/** Retry of a failed read: the page says it reads again until the answer */
export function retryCliTools(): void {
  cliToolsStore.setState({ readError: null })
  void refreshCliTools()
}

function setRunning(tool: CliTool, running: RunningCliToolAction | undefined): void {
  cliToolsStore.setState(state => ({ running: { ...state.running, [tool]: running } }))
}

/**
 * Runs `action` of `tool`, its lines shown where it was started (its row, or the line of the skill); then the toast of
 * what it did and the status again, which tells every window's Setup dot (`setup-changed`). `latest` names the release
 * an update wants, `server` the server a sign-in is to; an install brings the agent skill along when its checkbox is on
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
    const withSkill = action === "install" && cliToolsStore.getState().installWithSkill[tool]
    const result = await commands.runCliToolAction({ tool, action, latest, server, withSkill }, progress)
    if (result.status === "ok") {
      // Undo of a removed skill installs it again
      const undo = action === "removeSkill" ? { children: "Undo", onClick: () => void runCliToolAction(tool, "installSkill", null) } : undefined
      showToast({ title: result.data.title, action: undo })
      if (result.data.skill_problem !== null) {
        notifyError(`Cannot install the ${tool} agent skill`, result.data.skill_problem)
      }
    } else if (result.error.code !== "canceled") {
      notifyError(failedTitle(action, tool), result.error)
    }
  } catch (error) {
    notifyError(failedTitle(action, tool), error)
  } finally {
    setRunning(tool, undefined)
  }
  await refreshCliTools()
}

/**
 * Signs `youtrack` in with a token of the app's YouTrack sign-in (`status.signInWith`): `sign_in` of the `youtrack` CLI,
 * a one-shot sign-in in the browser, then Rust makes a permanent token and hands it to the CLI; the row waits meanwhile,
 * with the page to open, which opens when the sign-in waits for it
 */
export async function signInYoutrackWithApp(url: string): Promise<void> {
  const tool = "youtrack"
  if (cliToolsStore.getState().running[tool] !== undefined) {
    return
  }
  setRunning(tool, { action: "signIn", line: "Starting…", url: null })
  const onEvent = (event: SignInEvent) => {
    const page = cliToolsStore.getState().running[tool]?.url ?? null
    if (event.kind === "browser") {
      setRunning(tool, { action: "signIn", line: "Waiting for the browser…", url: event.url })
      void openSignInPage(event.url)
    } else if (event.kind === "waiting" && page !== null) {
      // the browser came back: the token is made and handed to the CLI
      setRunning(tool, { action: "signIn", line: "Signing youtrack in…", url: page })
    }
  }
  try {
    const result = await invokeSignIn({ provider: "youTrackCli", url }, onEvent)
    if (result.status === "ok") {
      showToast({ title: signedInTitle(result.data) })
    } else if (getErrorCode(result.error) !== "canceled") {
      notifyError(failedTitle("signIn", tool), result.error)
    }
  } finally {
    setRunning(tool, undefined)
  }
  await refreshCliTools()
}

/** Ends the action running for `tool`; its command answers the code `canceled`, which toasts nothing */
export function cancelCliToolAction(tool: CliTool): void {
  if (tool === "youtrack" && cliToolsStore.getState().running[tool]?.action === "signIn") {
    void cancelSignIn("youTrackCli")
    return
  }
  void commands.cancelCliToolAction(tool)
}

/** "Cannot install youtrack", "Cannot sign teamcity in" */
function failedTitle(action: CliToolAction, tool: CliTool): string {
  switch (action) {
    case "install":
      return `Cannot install ${tool}`
    case "update":
      return `Cannot update ${tool}`
    case "installSkill":
      return `Cannot install the ${tool} agent skill`
    case "updateSkill":
      return `Cannot update the ${tool} agent skill`
    case "removeSkill":
      return `Cannot remove the ${tool} agent skill`
    case "signIn":
      return `Cannot sign ${tool} in`
  }
}
