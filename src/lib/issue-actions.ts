import { openUrl } from "@tauri-apps/plugin-opener"
import { CircleCheckIcon, ExternalLinkIcon } from "lucide-react"
import type { YouTrackIssue } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { MenuItem } from "@/lib/menu"
import type { Operations } from "@/stores/operations"
import { operationItem } from "@/stores/operations"
import { issueSubject, markIssueFixed } from "@/stores/youtrack"

/**
 * The actions of a YouTrack issue (spec/repository/youtrack.spec.md), for every surface that renders them: the popover
 * of its key, the menu of its branch. Mark Fixed only for an open issue with a state field; disabled with the reason
 * when the field has no Fixed state.
 */
export function getIssueActions(issue: YouTrackIssue, running: Operations): MenuItem[] {
  const items: MenuItem[] = []
  const markFixed = getMarkFixedItem(issue.key, running)
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

/** "Mark ABC-1 Fixed", as the running operations leave it; it says what is checked when it runs */
export function getMarkFixedItem(key: string, running: Operations): MenuItem {
  const item: MenuItem = {
    id: "mark-fixed",
    label: "Mark Fixed",
    icon: CircleCheckIcon,
    description: `Sets the state of ${key} in YouTrack to Fixed`,
    onSelect: context => void markIssueFixed(key, context),
  }
  return operationItem(running, item, { id: "mark-fixed", subject: issueSubject(key), objects: [], kind: "remote" })
}
