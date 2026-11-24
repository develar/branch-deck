import type { UnattendedDenial, UnattendedHook, UnattendedMode } from "@/lib/bindings"
import type { ToastMessage } from "@/lib/toast-message"

/*
 * The words of Unattended Mode (openspec/specs/repository/unattended-mode/spec.md): what Rust answered of a switch
 * (`set_unattended_mode`) and of the requests its hook denied, worded for the title bar, the command palette, the
 * toasts and the list of Denied Requests.
 */

/** The item of the title bar and the palette, by the mode as it is: what a choice of it does */
export function unattendedModeLabel(on: boolean): string {
  return on ? "Turn Off Unattended Mode" : "Turn On Unattended Mode"
}

/** What the mode does while it is on: the description of the item and of the mark of the title bar */
export const UNATTENDED_DESCRIPTION = "While it is on, coding agents' permission requests are denied with a way on instead of waiting for you"

/** The mark of the mode in the title bar while it is on */
export const UNATTENDED_MARK = "Unattended"

/** The action of the toast of a switch off that opens the list */
export const SHOW_DENIED_REQUESTS = "Show Denied Requests…"

/** "1 permission request was denied", "3 permission requests were denied" */
export function deniedCount(count: number): string {
  if (count === 0) {
    return "No permission request was denied"
  }
  return count === 1 ? "1 permission request was denied" : `${count} permission requests were denied`
}

/** Why the switch on denies nothing yet: no hook of Branch Deck, or one of another Branch Deck */
function hookWarning(hook: UnattendedHook): string | null {
  switch (hook) {
    case "installed":
      return null
    case "missing":
      return "Claude Code has no hook of Unattended Mode, so its permission requests still wait for you: Install Agent CLI adds it"
    case "stale":
      return "The hook of Unattended Mode runs another Branch Deck, so it denies nothing: Install Agent CLI puts this one in its place"
  }
}

/** The line of Settings → Tools under brd while the hook is not this brd's */
export function unattendedHookLine(hook: UnattendedHook): string {
  return hook === "stale"
    ? "The hook of Unattended Mode runs another Branch Deck: Install Agent CLI puts this one in its place"
    : "The hook of Unattended Mode is not installed: Install Agent CLI adds it"
}

/**
 * The toast of a switch, none where the title bar says it in place: a switch on whose hook is installed, and a switch
 * that changed nothing. A switch on without the hook warns, a switch off tells how many requests the night denied
 */
export function unattendedToast(mode: UnattendedMode): Omit<ToastMessage, "action"> | null {
  if (!mode.changed) {
    return null
  }
  if (mode.on) {
    const warning = hookWarning(mode.hook)
    return warning === null ? null : { type: "warning", title: "Turned on Unattended Mode", description: warning }
  }
  return { title: "Turned off Unattended Mode", description: `${deniedCount(mode.denials.length)} while it was on` }
}

/** The toast of a switch offers Install Agent CLI: a switch on without the hook, where this host installs it */
export function offersInstall(mode: UnattendedMode): boolean {
  return mode.changed && mode.on && mode.hook !== "installed" && mode.installable
}

/**
 * The toast of a switch offers the list: a switch off after a night with denials, in the main window, which shows the
 * list (`listHere`); Settings tells the count alone
 */
export function offersDeniedRequests(mode: UnattendedMode, listHere = true): boolean {
  return listHere && mode.changed && !mode.on && mode.denials.length > 0
}

/** Who asked: "general-purpose (a7)" for a subagent of a kind, "Subagent a7" without one, "Main session" for the session itself */
export function deniedAgent(denial: UnattendedDenial): string {
  const id = denial.agent_id ?? null
  if (id === null) {
    return "Main session"
  }
  const kind = denial.agent_type ?? null
  return kind === null ? `Subagent ${id}` : `${kind} (${id})`
}

/** The last part of the working directory, which names the repository or the lease: `/pool/3/branch-deck` is `branch-deck` */
export function deniedPlace(denial: UnattendedDenial): string {
  const cwd = denial.cwd ?? ""
  return cwd.split(/[/\\]/).findLast(part => part !== "") ?? cwd
}
