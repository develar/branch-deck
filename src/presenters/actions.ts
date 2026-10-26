/**
 * The actions of a subject as Rust decides them (`RepositoryView.actions`, `domain_policy::actions`) in the page's look
 * and words: each action by its stable id has one look (`ACTION_LOOKS`: its label where no fact names it, its icon, its
 * key), and a blocker one sentence (`formatActionBlocker`). Every surface builds its items here (`presentAction`): the
 * "⋯" and right-click menus, the row's keys, the branch preview, the review window, the stack island and the command
 * palette, so an action is offered, named and refused alike everywhere. What runs on the page (an operation, a push) lies
 * over an item until the operations move into Rust (`operationItem`, s10); so does a sync of the table over a rewrite
 * (`whileSyncing`), which queues behind it then.
 */
import type { LucideIcon } from "lucide-react"
import {
  ArchiveXIcon,
  CirclePauseIcon,
  CirclePlayIcon,
  ClipboardCopyIcon,
  ClipboardListIcon,
  CombineIcon,
  DownloadIcon,
  ExternalLinkIcon,
  FileDiffIcon,
  FlaskConicalIcon,
  FolderInputIcon,
  GitPullRequestArrowIcon,
  GitPullRequestCreateIcon,
  GitPullRequestIcon,
  Layers2Icon,
  LayersIcon,
  PencilIcon,
  PenLineIcon,
  RefreshCwIcon,
  RotateCcwIcon,
  ShieldCheckIcon,
  ShieldOffIcon,
  SlidersHorizontalIcon,
  TagIcon,
  UngroupIcon,
  UploadIcon,
  UserPlusIcon,
} from "lucide-react"
import type { ActionBlocker, ActionId, ActionState, NothingBeyond } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import { formatAskFirst } from "@/lib/stack"
import { formatPendingFact, formatProblemsFailure, formatStepBlocker } from "@/presenters/next-step"
import { formatLanded, formatReason, formatRobotsLanded, formatStackHold } from "@/presenters/reasons"

/** How an action looks on every surface: its label where no fact names it ("Push (2)" does), its icon, its key */
export interface ActionLook {
  readonly label: string
  readonly icon: LucideIcon
  readonly kbds?: readonly string[]
}

/** The look of every action Rust offers, by id */
export const ACTION_LOOKS = {
  "safe-push": { label: "Safe Push", icon: ShieldCheckIcon, kbds: ["s"] },
  "safe-push-with-profile": { label: "Safe Push With Profile…", icon: SlidersHorizontalIcon },
  "dry-run": { label: "Dry Run", icon: FlaskConicalIcon },
  "check-approvals": { label: "Check Approvals Again", icon: RefreshCwIcon },
  "safe-push-without-review": { label: "Safe Push Without Review…", icon: ShieldOffIcon },
  push: { label: "Push", icon: UploadIcon, kbds: ["p"] },
  "push-with-children": { label: "Push with Children", icon: UploadIcon },
  "pick-remote-commits": { label: "Pick Remote Commits…", icon: DownloadIcon },
  "review-changes": { label: "Review Changes", icon: FileDiffIcon, kbds: ["d"] },
  "open-pull-request": { label: "Open Pull Request", icon: GitPullRequestIcon, kbds: ["o"] },
  "request-review": { label: "Request Review…", icon: UserPlusIcon },
  "retarget-pull-request": { label: "Retarget", icon: GitPullRequestArrowIcon },
  "create-pull-request": { label: "Create Pull Request…", icon: GitPullRequestCreateIcon },
  rename: { label: "Rename…", icon: PencilIcon },
  "move-to": { label: "Move to…", icon: FolderInputIcon, kbds: ["m"] },
  "stack-on": { label: "Stack On…", icon: LayersIcon },
  unstack: { label: "Unstack", icon: Layers2Icon },
  ungroup: { label: "Ungroup", icon: UngroupIcon },
  "amend-changes": { label: "Amend Changes…", icon: PenLineIcon },
  "add-issue-reference": { label: "Add Issue Reference…", icon: TagIcon },
  "squash-fixups": { label: "Squash Fixups", icon: CombineIcon },
  unapply: { label: "Unapply…", icon: ArchiveXIcon },
  park: { label: "Park Branch…", icon: CirclePauseIcon },
  unpark: { label: "Unpark Branch", icon: CirclePlayIcon },
  "safe-push-whole-stack": { label: "Safe Push Whole Stack", icon: ShieldCheckIcon },
  "dry-run-whole-stack": { label: "Dry Run Whole Stack", icon: FlaskConicalIcon },
  "safe-push-main-line": { label: "Safe Push", icon: ShieldCheckIcon },
  "open-stack-pull-request": { label: "Open Pull Request", icon: GitPullRequestIcon },
  "create-stack-pull-request": { label: "Create Pull Request of the Stack…", icon: GitPullRequestCreateIcon },
  "review-whole-stack": { label: "Review Whole Stack", icon: FileDiffIcon },
  "restart-robot": { label: "Restart Safe Push", icon: RotateCcwIcon },
  "push-and-restart-robot": { label: "Restart Safe Push", icon: RotateCcwIcon },
  "cancel-robot": { label: "Cancel Safe Push", icon: ShieldOffIcon },
  "copy-robot-prompt": { label: "Copy Prompt for Agent", icon: ClipboardCopyIcon },
  "copy-robot-full-prompt": { label: "Copy Prompt with CI Details", icon: ClipboardListIcon },
  "open-robot": { label: "Open in Patronus", icon: ExternalLinkIcon },
  "open-robot-pull-request": { label: "Open Pull Request", icon: GitPullRequestIcon },
} as const satisfies Record<ActionId, ActionLook>

/** What the words of a blocker read beside it, of the page: the subject it refuses and what the page shows of it */
export interface BlockerWords {
  /** The branch the action acts on (of a whole stack its root) */
  readonly subject: string
  /** The status text of the row of the branch (`Branch.statusText`): why its sync failed */
  readonly statusText?: string
  /**
   * The words of a reason of the next step of the subject (`BranchNextStep.safePushBlocker`, `dryRunBlocker`, the
   * blocker of its start without review): the step words its own reasons by its kind
   */
  readonly step?: string | null
}

/** Why Push with Children of `name` pushes nothing beyond the branch itself */
function nothingBeyondWords(name: string, why: NothingBeyond): string {
  switch (why.kind) {
    case "askFirst":
      return formatAskFirst(why.names)
    case "onlyBranch":
      return `Only ${name} has commits to push: its own push sends them`
    case "landed":
      return `${formatRobotsLanded(why.by, why.own, why.target)}: nothing of ${name} and its children is left to push`
    case "stackBranch":
      switch (why.along.kind) {
        case "notBuilt":
          return `The branches of the stack are on the remote. ${formatReason(why.along.reason)}`
        case "held":
          return `The branches of the stack are on the remote. ${formatStackHold(why.along.reason)}`
        case "dropsRemote":
          return "The branches of the stack are on the remote. The whole stack branch stays: its push would drop remote commits it lacks"
        default:
          return "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack"
      }
    case "onRemote":
      return "Nothing to push: the stack is on the remote"
  }
}

/** Why the branch failed to sync, as the action `id` says it: the push names the status of its row, a rewrite its stack problem */
function syncFailedWords(id: ActionId, problem: string | null, words: BlockerWords): string {
  if (id === "push") {
    const status = words.statusText ?? ""
    return status === "" ? "The branch failed to sync" : `The branch failed to sync: ${status}`
  }
  return problem ?? "The branch failed to sync"
}

/** "The branch has no commits", as the action `id` says it */
function noCommitsWords(id: ActionId): string {
  switch (id) {
    case "push":
      return "No commits to push"
    case "review-changes":
    case "review-whole-stack":
      return "No commits to review"
    default:
      return "The branch has no commits"
  }
}

/** The words of a sync of the table that a rewrite waits for */
export const SYNC_WAIT = "Wait for the sync to finish"

/** Why nobody is asked to review a draft (`PullRequestView.draft`) */
export function formatDraftReason(number: number): string {
  return `#${number} is a draft: mark it ready for review on GitHub first`
}

type BlockerFormatters = {
  readonly [C in ActionBlocker["code"]]: (blocker: Extract<ActionBlocker, { code: C }>, id: ActionId, words: BlockerWords) => string
}

const BLOCKERS = {
  rule: ({ reason }, _id, words) => formatReason(reason, () => words.statusText ?? ""),
  step: ({ blocker }, _id, words) => words.step ?? formatStepBlocker(blocker),
  pending: ({ fact }) => formatPendingFact(fact),
  problemsFailed: ({ message }) => formatProblemsFailure(message),
  syncing: () => SYNC_WAIT,
  syncFailed: ({ problem }, id, words) => syncFailedWords(id, problem, words),
  noCommits: (_blocker, id) => noCommitsWords(id),
  upToDate: () => "The remote branch is up to date",
  draft: ({ number }) => formatDraftReason(number),
  commentRoute: ({ number }) => `A comment on #${number} runs the default checks: it can't choose a profile`,
  allReferenced: () => "Every commit of the branch has an issue reference",
  parkHeld: ({ reason }) => `${formatReason(reason)}: it lands whatever a park says`,
  nothingBeyond: ({ why }, _id, words) => nothingBeyondWords(words.subject, why),
  landed: ({ landed }) => formatLanded(landed),
} satisfies BlockerFormatters as BlockerFormatters

/** The page's words of why the action `id` can't run now */
export function formatActionBlocker(id: ActionId, blocker: ActionBlocker, words: BlockerWords): string {
  return (BLOCKERS[blocker.code] as (blocker: ActionBlocker, id: ActionId, words: BlockerWords) => string)(blocker, id, words)
}

/** The state of the action `id` among `actions` (a subject's, `RepositoryView.actions`); undefined while it isn't offered */
export function findAction(actions: readonly ActionState[] | undefined, id: ActionId): ActionState | undefined {
  return actions?.find(action => action.id === id)
}

/** Why the action `id` can't run now, in the page's words; null while it can, undefined while the subject doesn't offer it */
export function actionBlocker(actions: readonly ActionState[] | undefined, id: ActionId, words: BlockerWords): string | null | undefined {
  const action = findAction(actions, id)
  if (action === undefined) {
    return undefined
  }
  return action.availability.kind === "enabled" ? null : formatActionBlocker(id, action.availability.reason, words)
}

/** The subject offers the action `id` and it can run now */
export function isActionEnabled(actions: readonly ActionState[] | undefined, id: ActionId): boolean {
  return findAction(actions, id)?.availability.kind === "enabled"
}

/** What a surface gives an action beside its look: what it runs, and what the facts name of it */
export interface ActionItem {
  readonly onSelect: MenuItem["onSelect"]
  /** The label the facts name ("Push (2)", "Unstack from auth"); the look's otherwise */
  readonly label?: string
  /** What it does, shown while it can run */
  readonly description?: string
  /** The keys of the surface; the look's otherwise, none where the key doesn't work there */
  readonly kbds?: readonly string[] | null
  readonly children?: MenuItem[]
  readonly compactLabel?: string
}

/**
 * The item of the action `id` of a subject (`actions`): its look, what the surface gives, disabled with the words of why
 * it can't run; null while the subject doesn't offer it
 */
export function presentAction(actions: readonly ActionState[] | undefined, id: ActionId, item: ActionItem, words: BlockerWords): MenuItem | null {
  const blocker = actionBlocker(actions, id, words)
  if (blocker === undefined) {
    return null
  }
  const look: ActionLook = ACTION_LOOKS[id]
  const kbds = item.kbds === null ? undefined : (item.kbds ?? look.kbds)
  const description = blocker ?? item.description
  return {
    id,
    label: item.label ?? look.label,
    icon: look.icon,
    onSelect: item.onSelect,
    ...(kbds !== undefined && { kbds: [...kbds] }),
    ...(blocker !== null && { disabled: true }),
    ...(description !== undefined && { description }),
    ...(item.children !== undefined && { children: item.children }),
    ...(item.compactLabel !== undefined && { compactLabel: item.compactLabel }),
  }
}

/**
 * A rewrite while a sync of the table runs: it waits for the sync (a quiet resync of a row that shows its last state
 * too), which Rust doesn't send with every row; an item disabled already keeps its reason
 */
export function whileSyncing(item: MenuItem, syncing: boolean): MenuItem {
  return syncing && item.disabled !== true ? { ...item, disabled: true, description: SYNC_WAIT } : item
}
