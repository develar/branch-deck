import { openUrl } from "@tauri-apps/plugin-opener"
import { CircleCheckIcon, ExternalLinkIcon } from "lucide-react"
import type { RunningOperation, YouTrackIssue, YouTrackStateChange } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showEventToast, toastKeys } from "@/lib/event-toast"
import type { MenuItem } from "@/lib/menu"
import { runFailure, runIntent } from "@/model/intents"
import { sessionRepositoryPath } from "@/model/store"
import { FIXED_STATE, loadIssue } from "@/stores/youtrack-issues"

/**
 * The actions of a YouTrack issue (spec/repository/youtrack.spec.md), for every surface that renders them: the popover
 * of its key, the menu of its branch. Mark Fixed only for an open issue with a state field; disabled with the reason
 * when the field has no Fixed state. `operations` are those of the session of the window (`RepositoryView.operations`):
 * a change of the issue that runs there shows on its item.
 */
export function getIssueActions(issue: YouTrackIssue, operations: Readonly<Record<string, RunningOperation>>): MenuItem[] {
  const items: MenuItem[] = []
  const markFixed = getMarkFixedItem(issue.key, operations)
  if (issue.state !== null && !issue.state.resolved) {
    items.push(
      issue.state.hasFixed
        ? markFixed
        : { ...markFixed, disabled: true, description: `${issue.state.field} of ${issue.key.slice(0, issue.key.lastIndexOf("-"))} has no Fixed state` },
    )
  }
  items.push({
    id: "open-in-youtrack",
    label: "Open in YouTrack",
    icon: ExternalLinkIcon,
    onSelect: () => void openUrl(issue.url).catch((error: unknown) => notifyError("Cannot open YouTrack", error)),
  })
  return items
}

/** "Mark ABC-1 Fixed", as the operations of the session leave it: a change of the issue that runs there says so */
export function getMarkFixedItem(key: string, operations: Readonly<Record<string, RunningOperation>>): MenuItem {
  const item: MenuItem = {
    id: "mark-fixed",
    label: "Mark Fixed",
    icon: CircleCheckIcon,
    description: `Sets the state of ${key} in YouTrack to Fixed`,
    onSelect: () => void markIssueFixed(key),
  }
  const running = Object.values(operations).find(({ intent }) => intent.kind === "setIssueState" && intent.key === key)
  if (running?.intent.kind !== "setIssueState") {
    return item
  }
  const description = running.intent.state === FIXED_STATE ? `Marking ${key} Fixed…` : `Setting ${key} to ${running.intent.state}…`
  return { ...item, running: true, disabled: true, description }
}

/**
 * Sets the state of the issue to Fixed (`SetIssueState` in the session of the window: a second click joins it), then
 * "Marked ABC-1 Fixed" with Undo, the same intent back to the state it had while the issue is still Fixed; the toasts
 * of the change and of its undo are one, the issue's
 */
async function markIssueFixed(key: string): Promise<void> {
  const change = await setIssueState(key, FIXED_STATE, null, `Cannot mark ${key} fixed`)
  if (change === null) {
    return
  }
  const previous = change.previous
  showEventToast({
    key: toastKeys.issue(key),
    render: () => ({
      type: "success",
      title: `Marked ${key} Fixed`,
      ...(previous !== null && { description: `Was ${previous}`, action: { label: "Undo", onClick: () => void undoMarkFixed(key, previous) } }),
    }),
  })
}

async function undoMarkFixed(key: string, previous: string): Promise<void> {
  if ((await setIssueState(key, previous, FIXED_STATE, `Cannot undo the Fixed state of ${key}`)) !== null) {
    showEventToast({ key: toastKeys.issue(key), render: () => ({ type: "success", title: `${key} is ${previous} again` }) })
  }
}

/**
 * The state of the issue `key` goes to `state` while it is `expected` (`SetIssueState`); the popover asks the issue
 * again either way. The change, or null when it failed (an error toast `failure` says why)
 */
async function setIssueState(key: string, state: string, expected: string | null, failure: string): Promise<YouTrackStateChange | null> {
  const repositoryPath = sessionRepositoryPath()
  if (repositoryPath === null) {
    notifyError(failure, "No repository is open")
    return null
  }
  const run = await runIntent(repositoryPath, { kind: "setIssueState", key, state, expected })
  void loadIssue(key)
  const error = runFailure(run, `The repository has no session to set ${key} in`)
  if (error !== null) {
    notifyError(failure, error)
    return null
  }
  return run.kind === "ran" && run.outcome.result.kind === "issue" ? run.outcome.result : null
}
