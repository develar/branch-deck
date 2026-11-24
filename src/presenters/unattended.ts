import type { UnattendedDenial, UnattendedMode } from "@/lib/bindings"
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

/**
 * The toast of a switch, none where the title bar says it in place: a switch on, and a switch that changed nothing. A
 * switch off tells how many requests the night denied, and warns when the hook stayed in the settings of Claude Code
 */
export function unattendedToast(mode: UnattendedMode): Omit<ToastMessage, "action"> | null {
  if (!mode.changed || mode.on) {
    return null
  }
  const description = `${deniedCount(mode.denials.length)} while it was on`
  const warning = mode.warning ?? null
  return warning === null
    ? { title: "Turned off Unattended Mode", description }
    : { type: "warning", title: "Turned off Unattended Mode", description: `${description}. ${warning}; it denies nothing while the mode is off` }
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
