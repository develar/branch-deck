/**
 * The mirror of the view of a repository (docs/design/ide-embedding.md, key decisions 1 to 3): `brd mcp` keeps the
 * state of a mirror channel of `air-ide` as the `RepositoryView` of its session, one part per field of the view, and
 * sends each `ViewPatch` as one `x-air-app/viewChanged` by the table rule of Air: `part` replaces a part, `set` puts one
 * entry of a keyed part, `remove` deletes one. The page reads the patches of its session (`subscribe_view`), so this
 * module turns the ops of a change back into the `TableChanges` of a `ViewPatch` from the state before them. It knows
 * the shape of the view, never a rule of it: what changed is what the ops name, compared by value where a part holds
 * keyed entries of its own (the sections, the Patronus view, the facts).
 */
import type {
  ArchivedChanges,
  FactsChanges,
  FactsView,
  PatronusChange,
  PatronusPart,
  PullRequestsChange,
  PullRequestsView,
  RepositoryView,
  RobotsChange,
  RobotsView,
  TableChanges,
  TableSections,
  Version,
  ViewPatch,
} from "@/lib/bindings"
import { withoutKeys } from "@/lib/record"

/** One op of `x-air-app/viewChanged` (`ViewOp` of `x-air-app.schema.json`) */
export type ViewOp =
  | { readonly op: "part"; readonly part: string; readonly value: unknown }
  | { readonly op: "set"; readonly part: string; readonly key: string; readonly value: unknown }
  | { readonly op: "remove"; readonly part: string; readonly key: string }

/** The state of a mirror: the parts of the view by name */
export type MirrorState = Readonly<Record<string, unknown>>

type Entries<T = unknown> = Readonly<Record<string, T>>

/** The keyed parts of the view that a patch changes by key, each with the field of `TableChanges` that removes a key */
const KEYED_PARTS = {
  branches: "removedBranches",
  stacks: "removedStacks",
  pushes: "removedPushes",
  openPullRequests: "removedOpenPullRequests",
  lands: "removedLands",
  landedStates: "removedLandedStates",
  nextSteps: "removedNextSteps",
  actions: "removedActions",
  attention: "removedAttention",
  githubLayers: "removedGithubLayers",
  reviewGroups: "removedReviewGroups",
  approvalLanding: "removedApprovalLanding",
  operations: "removedOperations",
} as const satisfies Partial<Record<keyof RepositoryView, keyof TableChanges>>

type KeyedPart = keyof typeof KEYED_PARTS

/** The parts a patch replaces whole, under the same name in `TableChanges` */
const WHOLE_PARTS = ["cached", "synced", "sync", "header", "order", "undo", "reviews", "route"] as const satisfies readonly (keyof RepositoryView & keyof TableChanges)[]

function isKeyed(part: string): part is KeyedPart {
  return Object.hasOwn(KEYED_PARTS, part)
}

function same(a: unknown, b: unknown): boolean {
  return a === b || JSON.stringify(a) === JSON.stringify(b)
}

/** The entries of `after` that differ from `before`, and the keys of `before` that `after` lacks */
function keyedDiff<T>(before: Entries<T> | undefined, after: Entries<T> | undefined): { changed: Record<string, T>; removed: string[] } {
  const changed: Record<string, T> = {}
  for (const [key, value] of Object.entries(after ?? {})) {
    if (before === undefined || !Object.hasOwn(before, key) || !same(before[key], value)) {
      changed[key] = value
    }
  }
  const removed = Object.keys(before ?? {}).filter(key => after === undefined || !Object.hasOwn(after, key))
  return { changed, removed }
}

/** The state after `ops`, by the table rule: the state before is left as it is */
export function applyOps(state: MirrorState, ops: readonly ViewOp[]): MirrorState {
  const next: Record<string, unknown> = { ...state }
  const copied = new Set<string>()
  for (const op of ops) {
    if (op.op === "part") {
      next[op.part] = op.value
      copied.delete(op.part)
      continue
    }
    const entries = (next[op.part] ?? {}) as Entries
    if (op.op === "remove") {
      next[op.part] = withoutKeys(entries, [op.key])
      copied.add(op.part)
      continue
    }
    if (!copied.has(op.part)) {
      next[op.part] = { ...entries }
      copied.add(op.part)
    }
    ;(next[op.part] as Record<string, unknown>)[op.key] = op.value
  }
  return next
}

/**
 * The version of a state: the `version` part of `brd mcp`; without one, the version a page numbers by the `serverSeq`
 * of `air-ide` in an epoch of its own
 */
export function versionOf(state: MirrorState, fallback: Version): Version {
  const version = state.version as Version | undefined
  return version !== undefined && typeof version.epoch === "number" && typeof version.seq === "number" ? version : fallback
}

/** The view a state holds at `version` */
export function viewOf(state: MirrorState, version: Version): RepositoryView {
  return { ...(state as unknown as RepositoryView), version }
}

function sectionChanges(before: TableSections, after: TableSections, changes: TableChanges): void {
  if (!same(before.unassigned, after.unassigned)) {
    changes.unassigned = after.unassigned
  }
  if (!same(before.unapplied, after.unapplied)) {
    changes.unapplied = after.unapplied
  }
  if (!same(before.landed, after.landed)) {
    changes.landed = after.landed
  }
  if (!same(before.archived, after.archived)) {
    const { changed, removed } = keyedDiff(before.archived.integration, after.archived.integration)
    const archived: ArchivedChanges = { names: after.archived.names, newlyArchived: after.archived.newlyArchived, integration: changed }
    if (removed.length > 0) {
      archived.removedIntegration = removed
    }
    changes.archived = archived
  }
}

function patronusChange(before: PatronusPart, after: PatronusPart): PatronusChange {
  const branches = keyedDiff(before.view.branches, after.view.branches)
  const stacks = keyedDiff(before.view.stacks, after.view.stacks)
  return {
    state: after.state,
    landingNeeds: after.landingNeeds,
    branches: branches.changed,
    removedBranches: branches.removed,
    stacks: stacks.changed,
    removedStacks: stacks.removed,
    restarts: after.view.restarts,
  }
}

function robotsChange(before: RobotsView, after: RobotsView): RobotsChange {
  const { changed, removed } = keyedDiff(before.robots, after.robots)
  return { head: after.head, robots: changed, removedRobots: removed, failing: after.failing, needsReview: after.needsReview, problemsFailed: after.problemsFailed }
}

function pullRequestsChange(before: PullRequestsView, after: PullRequestsView): PullRequestsChange {
  const pullRequests = keyedDiff(before.pullRequests, after.pullRequests)
  const stackPullRequests = keyedDiff(before.stackPullRequests, after.stackPullRequests)
  // the repository and the permission are taken with the head (`patchedPullRequests` of src/model/patch.ts)
  return {
    ...after,
    pullRequests: pullRequests.changed,
    removedPullRequests: pullRequests.removed,
    stackPullRequests: stackPullRequests.changed,
    removedStackPullRequests: stackPullRequests.removed,
  }
}

function factsChange(before: FactsView, after: FactsView): FactsChanges {
  const change: FactsChanges = {}
  if (!same(before.robots, after.robots)) {
    change.robots = robotsChange(before.robots, after.robots)
  }
  if (!same(before.pullRequests, after.pullRequests)) {
    change.pullRequests = pullRequestsChange(before.pullRequests, after.pullRequests)
  }
  for (const part of ["rules", "profiles", "reviewRequests", "reviewReadings", "auth"] as const) {
    if (!same(before[part], after[part])) {
      Object.assign(change, { [part]: after[part] })
    }
  }
  const readiness = keyedDiff(before.readiness, after.readiness)
  change.readiness = readiness.changed
  change.removedReadiness = readiness.removed
  const robotDetails = keyedDiff(before.robotDetails, after.robotDetails)
  change.robotDetails = robotDetails.changed
  change.removedRobotDetails = robotDetails.removed
  return change
}

/**
 * The patch of the session that `ops` are, from the view of `before` (at `base`) to that of `after` (at `next`): the
 * entries the ops set or remove of a keyed part, each part they replace under its field of `TableChanges`, and the
 * news of the part `news` whole (the page tells only those after the last it told, by id)
 */
export function patchOf(before: MirrorState, after: MirrorState, ops: readonly ViewOp[], base: Version, next: Version): ViewPatch {
  const changes: TableChanges = {}
  const replaced = new Set<string>()
  for (const op of ops) {
    if (op.op === "part") {
      replaced.add(op.part)
    } else if (isKeyed(op.part) && !replaced.has(op.part)) {
      const target = ((changes as Record<string, unknown>)[op.part] ??= {}) as Entries
      const removedField = KEYED_PARTS[op.part]
      const removed = ((changes as Record<string, unknown>)[removedField] ??= []) as string[]
      if (op.op === "set") {
        ;(target as Record<string, unknown>)[op.key] = op.value
        // a key removed and set again in one change is set
        const at = removed.indexOf(op.key)
        if (at >= 0) {
          removed.splice(at, 1)
        }
      } else {
        ;(changes as Record<string, unknown>)[op.part] = withoutKeys(target, [op.key])
        removed.push(op.key)
      }
    }
  }
  const view = before as unknown as RepositoryView
  const nextView = after as unknown as RepositoryView
  for (const part of replaced) {
    if (isKeyed(part)) {
      const { changed, removed } = keyedDiff(before[part] as Entries | undefined, after[part] as Entries | undefined)
      Object.assign(changes, { [part]: changed, [KEYED_PARTS[part]]: removed })
    } else if ((WHOLE_PARTS as readonly string[]).includes(part)) {
      Object.assign(changes, { [part]: after[part] })
    } else if (part === "sections") {
      sectionChanges(view.sections, nextView.sections, changes)
    } else if (part === "patronus") {
      changes.patronus = patronusChange(view.patronus, nextView.patronus)
    } else if (part === "facts") {
      changes.facts = factsChange(view.facts, nextView.facts)
    } else if (part === "news") {
      changes.news = nextView.news
    }
    // `version` is `next`; a part the page does not know is no part of its view
  }
  return { base, next, changes }
}
