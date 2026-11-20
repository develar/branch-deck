import type { FixComponent, FixHold, FixStep } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { formatLandingSubject } from "@/lib/patronus"

/** The subject of a commit ahead of the baseline without its prefix, by its original id; undefined when unknown */
export type SubjectOf = (id: string) => string | undefined

/**
 * A step of a fix as the Move action says it: "Move “Add login” to auth", "Move 3 commits of auth to login",
 * "Stack login on auth".
 */
export function describeFixStep(step: FixStep, subjectOf: SubjectOf): string {
  if (step.kind === "restack") {
    return `Stack ${step.fixes} on ${step.to}`
  }
  const subject = step.commits.length === 1 ? subjectOf(step.commits[0]!) : undefined
  if (subject !== undefined) {
    const unassigned = step.kind === "pullIn" && step.from.length === 0 ? "the unassigned " : ""
    return `Move ${unassigned}“${subject}” to ${step.to}`
  }
  const count = step.commits.length
  if (step.from.length === 0) {
    return `Move ${count} unassigned ${count === 1 ? "commit" : "commits"} to ${step.to}`
  }
  return `Move ${formatCommitCount(count)} of ${joinNames(step.from)} to ${step.to}`
}

/**
 * What else a fix changes for good: the issue groups it writes, so that the next commits of an issue follow ("Future
 * IJAI-4085 commits go to IJAI-4047 too")
 */
export function describeFixGrouping(component: FixComponent): string[] {
  return Object.entries(component.plan.issue_groups ?? {}).flatMap(([branch, keys]) => keys.map(key => `Future ${key} commits go to ${branch} too`))
}

/**
 * The result of a fix in one paragraph: "Afterwards login and auth apply cleanly, and docs syncs again. legacy has no
 * commits left." `hasPullRequest` names the branches with an open pull request: the next push changes them
 */
export function describeFixResult(component: FixComponent, hasPullRequest: (branch: string) => boolean = () => false): string {
  const sentences: string[] = []
  if (component.fixed.length > 0) {
    let sentence = `Afterwards ${joinNames(component.fixed)} ${component.fixed.length === 1 ? "applies" : "apply"} cleanly`
    if (component.unblocked.length > 0) {
      sentence += `, and ${joinNames(component.unblocked)} ${component.unblocked.length === 1 ? "syncs" : "sync"} again`
    }
    sentences.push(`${sentence}.`)
  }
  for (const branch of component.emptied) {
    // a branch whose commits all moved: say where
    const to = [...new Set(component.steps.filter(step => step.kind !== "restack" && step.from.includes(branch)).map(step => step.to))]
    const where = to.length === 0 ? "" : `: its commits are in ${joinNames(to)}`
    sentences.push(`${branch} has no commits left${where}${hasPullRequest(branch) ? "; its pull request stays open" : ""}.`)
  }
  if (component.orphanedChildren.length > 0) {
    const orphans = component.orphanedChildren
    sentences.push(`${joinNames(orphans)} ${orphans.length === 1 ? "is" : "are"} built on the nearest branch below with commits.`)
  }
  const changed = branchesOfFix(component).filter(branch => !component.emptied.includes(branch) && hasPullRequest(branch))
  if (changed.length > 0) {
    sentences.push(`The next push updates the ${changed.length === 1 ? "pull request" : "pull requests"} of ${joinNames(changed)}.`)
  }
  return sentences.join(" ")
}

/** The branches a component changes: where commits come from and go to, and the restacked ones */
function branchesOfFix(component: FixComponent): string[] {
  return [...new Set(component.steps.flatMap(step => [...step.from, step.to, ...(step.kind === "restack" ? [step.fixes] : [])]))]
}

/** What the hold of a branch that holds what a fix brings says, above or in place of the fix */
export interface FixHoldWords {
  /** The callout's title while waiting is the way (`lands`, `landed`); null when the fix stays the title (`failing`) */
  readonly title: string | null
  readonly detail: string
}

/**
 * The words of the hold Rust decided for the branch `holder` that has what `fixed` need (`FixHold`,
 * `domain_policy::holds::fix_hold`): "Wait for the Safe Push of git-worktrees" ("It lands a2ui-v1, which has what
 * apps-context-root needs: once it merges, Update Project applies apps-context-root cleanly, without a fix."); "The
 * Safe Push of git-worktrees is failing: a2ui-v1 doesn't land with it, so the fix is the way"; "a2ui-v1 landed with
 * git-worktrees" ("Update Project applies apps-context-root cleanly.")
 */
export function describeFixHold(hold: FixHold, holder: string, fixed: readonly string[]): FixHoldWords {
  const subject = formatLandingSubject(hold.by)
  const own = hold.by.branch === holder && hold.by.wholeStack !== true
  const names = joinNames(fixed)
  switch (hold.kind) {
    case "lands": {
      const needs = fixed.length > 1 ? "need" : "needs"
      const after = `Update Project applies ${names} cleanly, without a fix.`
      return {
        title: `Wait for the Safe Push of ${subject}`,
        detail: own
          ? `${holder} has what ${names} ${needs}: once its Safe Push merges, ${after}`
          : `It lands ${holder}, which has what ${names} ${needs}: once it merges, ${after}`,
      }
    }
    case "failing":
      return {
        title: null,
        detail: own
          ? `The Safe Push of ${holder} is failing, so waiting for it lands nothing: fix the conflict now, or restart the Safe Push.`
          : `The Safe Push of ${subject} is failing, so ${holder} doesn't land with it: fix the conflict now, or restart the Safe Push.`,
      }
    case "landed":
      return {
        title: own ? `${holder} is merged by Patronus` : `${holder} landed with ${subject}`,
        detail: `Update Project applies ${names} cleanly, without a fix.`,
      }
  }
}

/** The toast title of an applied fix: "Fixed the conflict of auth", "Fixed the conflicts of 3 branches" */
export function fixTitle(component: FixComponent): string {
  const fixed = component.fixed
  if (fixed.length === 1) {
    return `Fixed the conflict of ${fixed[0]}`
  }
  return `Fixed the conflicts of ${fixed.length} branches`
}

/** The component of `components` that fixes `branch`: one of its steps is for it, or it fixes it on the way */
export function componentFixing(components: readonly FixComponent[], branch: string): FixComponent | undefined {
  return (
    components.find(component => component.steps.some(step => step.fixes === branch)) ??
    components.find(component => component.fixed.includes(branch) || component.unblocked.includes(branch))
  )
}

/** "a", "a and b", "a, b and c" */
export function joinNames(names: readonly string[]): string {
  if (names.length <= 1) {
    return names[0] ?? ""
  }
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`
}
