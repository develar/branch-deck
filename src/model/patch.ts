/**
 * The view of a session after a patch, as `repository_model::patch::apply_patch` makes it in Rust: the parts the patch
 * names replace theirs, the keyed parts (branches, stacks, the integration of the archived branches, the branches and the
 * stacks of the Patronus view) only for the keys
 * it names. The page's store (`src/model/store.ts`) applies the patches of its session with it, the fake sessions of
 * the store tests (`src/test-support/fake-sessions.ts`) keep their views with it
 */
import type {
  BranchChange,
  BranchView,
  FactsChanges,
  FactsView,
  PatronusChange,
  PatronusPart,
  PullRequestsChange,
  PullRequestsView,
  RepositoryView,
  RobotsChange,
  RobotsView,
  ViewPatch,
} from "@/lib/bindings"
import { withoutKeys } from "@/lib/record"

/** The view after `patch`; the caller checked that `view` is at its base */
export function applyViewPatch(view: RepositoryView, patch: ViewPatch): RepositoryView {
  const { changes } = patch
  let branches = view.branches
  const changedBranches = Object.entries(changes.branches ?? {})
  if (changedBranches.length > 0 || (changes.removedBranches?.length ?? 0) > 0) {
    branches = withoutKeys(branches, changes.removedBranches ?? [])
    for (const [name, change] of changedBranches) {
      const patched = patchedBranch(branches[name], change)
      if (patched !== undefined) {
        branches[name] = patched
      }
    }
  }
  let stacks = view.stacks
  if (Object.keys(changes.stacks ?? {}).length > 0 || (changes.removedStacks?.length ?? 0) > 0) {
    stacks = { ...withoutKeys(stacks, changes.removedStacks ?? []), ...changes.stacks }
  }
  const sections = view.sections
  const archived = changes.archived
  return {
    version: patch.next,
    sync: changes.sync ?? view.sync,
    header: changes.header ?? view.header,
    order: changes.order ?? view.order,
    branches,
    stacks,
    sections: {
      unassigned: changes.unassigned ?? sections.unassigned,
      unapplied: changes.unapplied ?? sections.unapplied,
      landed: changes.landed ?? sections.landed,
      archived:
        archived == null
          ? sections.archived
          : {
              names: archived.names ?? sections.archived.names,
              newlyArchived: archived.newlyArchived ?? sections.archived.newlyArchived,
              integration: { ...withoutKeys(sections.archived.integration, archived.removedIntegration ?? []), ...archived.integration },
            },
    },
    patronus: changes.patronus == null ? view.patronus : patchedPatronus(view.patronus, changes.patronus),
    facts: changes.facts == null ? view.facts : patchedFacts(view.facts, changes.facts),
    news: (changes.news?.length ?? 0) === 0 ? view.news : [...view.news, ...changes.news!].slice(-NEWS_KEPT),
  }
}

/** How many news a view keeps, as `domain_types::session::NEWS_KEPT` */
const NEWS_KEPT = 32

/** A keyed part after a patch: the entries it names replace theirs, those it removes go; the same object without either */
function keyed<T>(entries: Readonly<Record<string, T>>, changed: Readonly<Record<string, T>> | undefined, removed: readonly string[] | undefined): Readonly<Record<string, T>> {
  return Object.keys(changed ?? {}).length === 0 && (removed?.length ?? 0) === 0 ? entries : { ...withoutKeys(entries, removed ?? []), ...changed }
}

/** The facts after a patch: each part it names, the keyed ones by key */
function patchedFacts(facts: FactsView, change: FactsChanges): FactsView {
  return {
    robots: change.robots == null ? facts.robots : patchedRobots(facts.robots, change.robots),
    pullRequests: change.pullRequests == null ? facts.pullRequests : patchedPullRequests(facts.pullRequests, change.pullRequests),
    rules: change.rules ?? facts.rules,
    profiles: change.profiles ?? facts.profiles,
    readiness: keyed(facts.readiness, change.readiness, change.removedReadiness),
    robotDetails: keyed(facts.robotDetails, change.robotDetails, change.removedRobotDetails),
  }
}

/**
 * The pull requests after a patch: its head when it names one, the pull requests it names replace theirs, those it
 * removes go; one it doesn't name keeps its object, so a row that reads one doesn't render again
 */
function patchedPullRequests(part: PullRequestsView, change: PullRequestsChange): PullRequestsView {
  return {
    head: change.head ?? part.head,
    pullRequests: keyed(part.pullRequests, change.pullRequests, change.removedPullRequests),
    stackPullRequests: keyed(part.stackPullRequests, change.stackPullRequests, change.removedStackPullRequests),
  }
}

/** The robots after a patch: its head when it names one, the robots it names replace theirs, those it removes go */
function patchedRobots(robots: RobotsView, change: RobotsChange): RobotsView {
  return {
    head: change.head ?? robots.head,
    robots: { ...withoutKeys(robots.robots, change.removedRobots ?? []), ...change.robots },
    failing: change.failing ?? robots.failing,
    needsReview: change.needsReview ?? robots.needsReview,
    problemsFailed: change.problemsFailed ?? robots.problemsFailed,
  }
}

/**
 * The Patronus view after a patch: its states when it names them, the branches and the stacks it names replace theirs,
 * those it removes go; an entry it doesn't name keeps its object, so a row that reads one doesn't render again
 */
function patchedPatronus(part: PatronusPart, change: PatronusChange): PatronusPart {
  const { view } = part
  return {
    state: change.state ?? part.state,
    landingNeeds: change.landingNeeds ?? part.landingNeeds,
    view: {
      branches: keyed(view.branches, change.branches, change.removedBranches),
      stacks: keyed(view.stacks, change.stacks, change.removedStacks),
      landedByRobot: change.landedByRobot ?? view.landedByRobot,
    },
  }
}

/** A branch the patch changes; a new one comes with its head and commits, its other parts as a fresh row has them */
function patchedBranch(branch: BranchView | undefined, change: BranchChange): BranchView | undefined {
  if (branch !== undefined) {
    return {
      head: change.head ?? branch.head,
      commits: change.commits ?? branch.commits,
      sync: change.sync ?? branch.sync,
      copies: change.copies ?? branch.copies,
      remote: change.remote ?? branch.remote,
    }
  }
  if (change.head == null || change.commits == null) {
    return undefined
  }
  return {
    head: change.head,
    commits: change.commits,
    sync: change.sync ?? { status: "Syncing", processedCount: 0, hasError: false },
    copies: change.copies ?? change.commits.map(() => ({})),
    remote: change.remote ?? { status: null },
  }
}
