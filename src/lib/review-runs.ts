import { getCurrentWindow } from "@tauri-apps/api/window"
import { askOn } from "@/hooks/useMainWindowLink"
import type { ReviewRequest } from "@/lib/bindings"
import { ROW_FORM_ACTIONS } from "@/lib/bindings"
import { branchGoneReason, keptGoneReason } from "@/lib/branch-review"
import { logError, recordError } from "@/lib/errors"
import { closeEventToasts } from "@/lib/event-toast"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import type { ReviewToast } from "@/lib/pull-request-review"
import { findReviewItem, keptBranchName, keptOfSubject } from "@/lib/review-actions"
import { showRepository } from "@/lib/show-repository"
import { serveRequests, windowLinkEvents } from "@/lib/sub-window-link"
import type { LinkRequestEvents } from "@/lib/sub-window-link"
import { formatWholeStack } from "@/lib/whole-stack"
import { REVIEW_WINDOW } from "@/lib/window-label"
import { archivedBranchesStore } from "@/stores/archived-branches"
import { getBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import type { RevealRequest } from "@/stores/commit-move"
import { approveAndSafePush, markReviewRequestRead } from "@/stores/review-requests"
import { getUnappliedBranch } from "@/stores/unapplied-branches"

/*
 * The one request of the review window to the main window (openspec/specs/review/review-window/spec.md#actions): an action that
 * opens a form under the row of its subject (`ROW_FORM_ACTIONS` of Rust: a rename, a move, the popover of a Safe Push, the
 * question of a push that drops remote commits, …) runs there. The window builds every action itself from its own
 * session and runs all the others itself; the main window shows the repository of the request (`showRepository`), finds
 * the item by the same builders (`findReviewItem`), shows its row and runs it, or refuses it with why (`review/feedback`,
 * recorded in Activity).
 */

/** The events of the runs: `review/run`, its answer and its feedback */
export const REVIEW_RUN_EVENTS: LinkRequestEvents = { ...windowLinkEvents(REVIEW_WINDOW), request: "review/run" }

/** An action the review window asks the main window to run, at the row of its subject */
export interface ReviewRunRequest {
  readonly id: string
  readonly repositoryPath: string
  /** A branch by name, the whole stack of a root (`stack:<root>`), a kept branch (`unapplied:<full>`, `archived:<full>`) */
  readonly subject: string
  /** The id of its item, as the builders of the row name it */
  readonly itemId: string
  /** An item of the menu of these commits of the branch, by their ids in the virtual branch; of a pull request the reviewed commit */
  readonly commitIds?: readonly string[]
  /** Of a pull request (`pull:<number>`), the whole request: an approved one has left the list of the main window */
  readonly request?: ReviewRequest
}

/**
 * How a run went, shown in the window: why the main window refused it (it recorded it in Activity), or the toast of the
 * Safe Push of a pull request it started
 */
export type ReviewRunFeedback = { readonly kind: "error"; readonly title: string; readonly message: string } | { readonly kind: "toast"; readonly toast: ReviewToast }

/** The item of a pull request the window asks the main window for: its Safe Push, which Patronus answers there */
export const PULL_REQUEST_SAFE_PUSH = "safe-push-pull-request"

/** The item of a pull request the window shows (↑/↓, Open on GitHub): it is read in Your Turn */
export const PULL_REQUEST_READ = "read"

/** Items the builders offer that Rust's catalog doesn't name, which open their form at the row too */
const ROW_ITEMS: ReadonlySet<string> = new Set(["fix-conflict", "push-branch-only"])

const ROW_FORMS: ReadonlySet<string> = new Set([...ROW_FORM_ACTIONS, ...ROW_ITEMS])

/** Whether the item `id` opens a form under the row of its subject, so the review window asks the main window to run it */
export function opensRowForm(id: string | undefined): boolean {
  return id !== undefined && ROW_FORMS.has(id)
}

/** Asks the main window to run an item at its row; resolves to whether it did (a refusal is told by its feedback) */
export function askReviewRun(request: Omit<ReviewRunRequest, "id">): Promise<boolean> {
  return askOn<ReviewRunRequest>(REVIEW_RUN_EVENTS, request)
}

/** What a run does, for "Cannot …": "push branch auth", "start the Safe Push of auth", `run "Rename" on auth` */
function runAction(itemId: string, label: string, subject: string): string {
  switch (itemId) {
    case "push":
      return `push branch ${subject}`
    case "safe-push":
    case "safe-push-without-review":
      return `start the Safe Push of ${subject}`
    case "push-to-baseline":
      return `push ${subject} into the target`
    case "merge-pull-request":
      return `merge the pull request of ${subject}`
    default:
      return `run "${label}" on ${subject}`
  }
}

/** How the subject of a run is named in a refusal: "auth", "the whole air stack", "a1b2c3d4" of a commit */
function subjectName(request: ReviewRunRequest): string {
  const kept = keptOfSubject(request.subject)
  if (kept !== null) {
    return keptBranchName(kept)
  }
  if (request.subject.startsWith("stack:")) {
    return formatWholeStack(request.subject.slice("stack:".length))
  }
  const commitId = request.commitIds?.[0]
  return commitId === undefined ? request.subject : formatShortHash(commitId)
}

/**
 * Runs what the review window asks, in the main window, which has the rows: switches to the repository of the request
 * first, shows the row of the subject (at the commit of a commit's item) and runs the item as the row's menu would; one
 * it can't run now (the subject left, the item is disabled or gone) is refused with the reason, shown in the window. An
 * item whose intent runs in the session already is that start (a click in both windows runs it once). Main window only.
 */
export const runReviewRequest = serveRequests<ReviewRunRequest, ReviewRunFeedback>(REVIEW_RUN_EVENTS, REVIEW_WINDOW, async (request, send) => {
  if (request.request !== undefined) {
    return runPullRequestRequest(request, request.request, send)
  }
  const commitId = request.commitIds?.[0] ?? null
  const label = (findReviewItem(request.subject, request.itemId, commitId)?.label ?? request.itemId).replace(/…$/, "")
  const title = `Cannot ${runAction(request.itemId, label, subjectName(request))}`
  const refuse = (reason: unknown): false => {
    send({ kind: "error", title, message: recordError(title, reason) })
    return false
  }
  const notShown = await showRepository(request.repositoryPath)
  if (notShown !== null) {
    return refuse(notShown)
  }
  const kept = keptOfSubject(request.subject)
  if (kept !== null) {
    const present = kept.kind === "unapplied" ? getUnappliedBranch(kept.name) !== undefined : archivedBranchesStore.getState().branches[kept.name] !== undefined
    if (!present) {
      return refuse(keptGoneReason(kept, keptBranchName(kept)))
    }
  } else {
    const name = request.subject.startsWith("stack:") ? request.subject.slice("stack:".length) : request.subject
    if (getBranch(name) === undefined) {
      return refuse(branchGoneReason(name))
    }
  }
  const item = findReviewItem(request.subject, request.itemId, commitId)
  if (item === undefined) {
    return refuse(
      commitId !== null && getBranch(request.subject)?.commits.every(it => it.hash !== commitId) === true
        ? `${request.subject} no longer has the commit: a sync rebuilt it`
        : `${subjectName(request)} has no such action now`,
    )
  }
  const commit = commitId === null ? undefined : getBranch(request.subject)?.commits.find(it => it.hash === commitId)
  const reveal: RevealRequest | null =
    kept !== null
      ? null
      : {
          branchName: request.subject.startsWith("stack:") ? request.subject.slice("stack:".length) : request.subject,
          commitIds: commit === undefined ? [] : [commit.originalHash],
        }
  return runItem(item, reveal, refuse)
})

/**
 * Runs what the review of a pull request asks of the main window: the read of the request in Your Turn, its Safe Push
 * (the approval first where it lacks, Patronus answers the comment, the robot watched here; `approveAndSafePush` shows
 * the repository first): its toast goes to the window, the toast of the request in the main window closes
 */
function runPullRequestRequest(request: ReviewRunRequest, pullRequest: ReviewRequest, send: (feedback: ReviewRunFeedback) => void): Promise<boolean> | boolean {
  if (request.itemId === PULL_REQUEST_READ) {
    markReviewRequestRead(request.repositoryPath, pullRequest)
    return true
  }
  const commitId = request.commitIds?.[0]
  if (request.itemId !== PULL_REQUEST_SAFE_PUSH || commitId === undefined) {
    send({ kind: "error", title: `Cannot run "${request.itemId}" on #${pullRequest.number}`, message: "The review has no such action" })
    return false
  }
  return approveAndSafePush(request.repositoryPath, pullRequest, commitId, {
    toast: toast => {
      closeEventToasts(toast.key)
      send({ kind: "toast", toast })
    },
    error: (title, error) => send({ kind: "error", title, message: recordError(title, error) }),
  })
}

/**
 * Runs an item the window asked for as the row would: the main window comes to the front and shows the row; one whose
 * intent runs in the session is that start already; a disabled one is refused
 */
function runItem(item: MenuItem, reveal: RevealRequest | null, refuse: (reason: unknown) => false): boolean {
  if (item.running === true) {
    return true
  }
  if (item.disabled === true) {
    return refuse(item.description ?? `${item.label} is not available now`)
  }
  getCurrentWindow()
    .setFocus()
    .catch((error: unknown) => logError("Cannot focus the main window", error))
  if (reveal !== null) {
    requestReveal(reveal)
  }
  item.onSelect(FROM_MENU)
  return true
}
