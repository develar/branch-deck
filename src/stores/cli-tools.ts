import { listen } from "@tauri-apps/api/event"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { useTauriCustomListen } from "@/hooks/useTauriListen"
import type { CliTool, CliToolAction, CliToolsRunning, CliToolStatus, SignInEvent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorCode, notifyError } from "@/lib/errors"
import { cancelSignIn, invokeSignIn, openSignInPage, signedInTitle } from "@/lib/sign-in-run"
import { showToast } from "@/lib/toast-message"

/*
 * The `teamcity` and `youtrack` CLIs of Settings → Tools (openspec/specs/repository/cli-tools/spec.md): the last status Rust
 * read, and the actions that run with the last line of each. Rust keeps the actions (`cli_tools_running`, the event
 * `cli-tools-running`), so every window shows one that another window started, or that ran before the page loaded; only
 * the sign-in of youtrack with the app's YouTrack, another command, is the action of the window that started it.
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
  /** The actions Rust runs, at the newest version a read or an event gave */
  readonly running: { readonly version: number; readonly byTool: Partial<Record<CliTool, RunningCliToolAction>> }
  /** The sign-in of youtrack with the app's YouTrack (`signInYoutrackWithApp`) that this window runs */
  readonly appSignIn: RunningCliToolAction | undefined
  /** "Also install the agent skill" of a missing CLI: on by default, kept while the app runs, not a setting */
  readonly installWithSkill: Record<CliTool, boolean>
}

const cliToolsStore = createStore<CliToolsState>(() => ({
  statuses: null,
  readError: null,
  running: { version: -1, byTool: {} },
  appSignIn: undefined,
  installWithSkill: { teamcity: true, youtrack: true },
}))

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

/** The action of `tool` that runs; none for no tool (an item of the Setup dot that is not of a CLI) */
export function useRunningCliToolAction(tool: CliTool | null): RunningCliToolAction | undefined {
  return useStore(cliToolsStore, state => (tool === null ? undefined : runningOf(state, tool)))
}

function runningOf(state: CliToolsState, tool: CliTool): RunningCliToolAction | undefined {
  return state.running.byTool[tool] ?? (tool === "youtrack" ? state.appSignIn : undefined)
}

/** Keeps `running` when it is newer than what the window has: a read and the events answer on different paths */
function acceptRunning(running: CliToolsRunning): void {
  cliToolsStore.setState(state => {
    if (running.version <= state.running.version) {
      return state
    }
    const byTool: Partial<Record<CliTool, RunningCliToolAction>> = {}
    for (const it of running.actions) {
      byTool[it.tool] = { action: it.action, line: it.line, url: it.url }
    }
    return { running: { version: running.version, byTool } }
  })
}

/**
 * Follows the actions of the tools that run, in every window that shows a tool (Settings → Tools, the Setup dot):
 * subscribes to `cli-tools-running` first, then reads them, so no end of an action falls between the two
 */
export function useCliToolsRunning(): void {
  useTauriCustomListen("cli-tools-running", async () => {
    const unlisten = await listen<CliToolsRunning>("cli-tools-running", event => acceptRunning(event.payload))
    void commands.cliToolsRunning().then(acceptRunning)
    return unlisten
  })
}

/**
 * Reads the status of every tool again. Rust answers a read asked while one runs with the read after it, which starts
 * after what changed (an action just ended, the user installed a tool in a terminal). A failure keeps the last status
 * and says why (`readError`)
 */
export async function refreshCliTools(): Promise<void> {
  const result = await commands.cliToolsStatus()
  cliToolsStore.setState(result.status === "ok" ? { statuses: result.data, readError: null } : { readError: result.error.message })
}

/** Retry of a failed read: the page says it reads again until the answer */
export function retryCliTools(): void {
  cliToolsStore.setState({ readError: null })
  void refreshCliTools()
}

/**
 * Runs `action` of `tool`, which every window then shows running (its row, or the line of the skill, and the item of the
 * Setup dot); then the toast of what it did and the status again, which tells every window's Setup dot (`setup-changed`).
 * `latest` names the release an update wants, `server` the server a sign-in is to; an install brings the agent skill
 * along when its checkbox is on. An action of the tool that runs already (`cli_tool_busy`: another window started it a
 * moment before) toasts nothing: the row shows it
 */
export async function runCliToolAction(tool: CliTool, action: CliToolAction, latest: string | null, server: string | null = null): Promise<void> {
  if (runningOf(cliToolsStore.getState(), tool) !== undefined) {
    return
  }
  try {
    const withSkill = action === "install" && cliToolsStore.getState().installWithSkill[tool]
    const result = await commands.runCliToolAction({ tool, action, latest, server, withSkill })
    if (result.status === "ok") {
      // Undo of a removed skill installs it again
      const undo = action === "removeSkill" ? { children: "Undo", onClick: () => void runCliToolAction(tool, "installSkill", null) } : undefined
      showToast({ title: result.data.title, action: undo })
      if (result.data.skill_problem !== null) {
        notifyError(`Cannot install the ${tool} agent skill`, result.data.skill_problem)
      }
    } else if (result.error.code !== "canceled" && result.error.code !== "cli_tool_busy") {
      notifyError(failedTitle(action, tool), result.error)
    }
  } catch (error) {
    notifyError(failedTitle(action, tool), error)
  }
  await refreshCliTools()
}

function setAppSignIn(running: RunningCliToolAction | undefined): void {
  cliToolsStore.setState({ appSignIn: running })
}

/**
 * Signs `youtrack` in with a token of the app's YouTrack sign-in (`status.signInWith`): `sign_in` of the `youtrack` CLI,
 * a one-shot sign-in in the browser, then Rust makes a permanent token and hands it to the CLI; the row waits meanwhile,
 * with the page to open, which opens when the sign-in waits for it
 */
export async function signInYoutrackWithApp(url: string): Promise<void> {
  const tool = "youtrack"
  if (runningOf(cliToolsStore.getState(), tool) !== undefined) {
    return
  }
  setAppSignIn({ action: "signIn", line: "Starting…", url: null })
  const onEvent = (event: SignInEvent) => {
    const page = cliToolsStore.getState().appSignIn?.url ?? null
    if (event.kind === "browser") {
      setAppSignIn({ action: "signIn", line: "Waiting for the browser…", url: event.url })
      void openSignInPage(event.url)
    } else if (event.kind === "waiting" && page !== null) {
      // the browser came back: the token is made and handed to the CLI
      setAppSignIn({ action: "signIn", line: "Signing youtrack in…", url: page })
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
    setAppSignIn(undefined)
  }
  await refreshCliTools()
}

/** Ends the action running for `tool`; its command answers the code `canceled`, which toasts nothing */
export function cancelCliToolAction(tool: CliTool): void {
  if (tool === "youtrack" && cliToolsStore.getState().running.byTool[tool] === undefined && cliToolsStore.getState().appSignIn !== undefined) {
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
