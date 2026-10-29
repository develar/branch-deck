import type { AutosquashPlan, AutosquashScope, FixupKind, OrphanFixup, PlannedGroup } from "@/lib/bindings"

/** The fixups of one list: a branch, or the unassigned commits (null) */
export interface ListFixups {
  /** Groups whose target is in the list */
  groups: readonly PlannedGroup[]
  /** Fixups of the list without a target */
  orphans: readonly OrphanFixup[]
  /** Fixup commits squashed by the groups */
  count: number
}

const NO_FIXUPS: ListFixups = { groups: [], orphans: [], count: 0 }

/** The groups and orphans of a branch (null: the unassigned commits); the plan's groups are by target branch. */
export function getListFixups(plan: AutosquashPlan | null, branch: string | null): ListFixups {
  if (plan === null) {
    return NO_FIXUPS
  }
  const groups = plan.groups.filter(group => group.branch === branch)
  const orphans = plan.orphans.filter(orphan => orphan.branch === branch)
  if (groups.length === 0 && orphans.length === 0) {
    return NO_FIXUPS
  }
  return { groups, orphans, count: groups.reduce((sum, group) => sum + group.folded.length, 0) }
}

export function getAutosquashScope(branch: string | null): AutosquashScope {
  return branch === null ? { type: "unassigned" } : { type: "branch", value: branch }
}

/** Where a fixup commit goes: its target group, or null for a fixup without a target */
export type FixupTarget = PlannedGroup | null

const targetIndexes = new WeakMap<AutosquashPlan, Map<string, FixupTarget>>()

/** The target of a fixup commit, null for an orphan; undefined if the plan does not know the commit. */
export function getFixupTarget(plan: AutosquashPlan | null, commitId: string): FixupTarget | undefined {
  if (plan === null) {
    return undefined
  }
  let index = targetIndexes.get(plan)
  if (index === undefined) {
    index = new Map()
    for (const group of plan.groups) {
      for (const fixup of group.folded) {
        index.set(fixup.commitId, group)
      }
    }
    for (const orphan of plan.orphans) {
      index.set(orphan.commitId, null)
    }
    targetIndexes.set(plan, index)
  }
  return index.get(commitId)
}

/** "Squash 1 Fixup…", "Squash 3 Fixups…": the button above the commits and the branch menu, which open the confirmation */
export function formatSquashFixupsLabel(count: number): string {
  return count === 1 ? "Squash 1 Fixup…" : `Squash ${count} Fixups…`
}

/**
 * What the squash of `count` fixups does, under the title of its confirmation: one fixup reads as one; null without a
 * fixup to squash, whose title says so
 */
export function describeSquashFixups(count: number): string | null {
  if (count === 0) {
    return null
  }
  const folded = count === 1 ? "It is folded into its target" : "Each one is folded into its target"
  return `${folded}, as with rebase --autosquash. Your files are not touched.`
}

/** "1 fixup", "3 fixups" */
export function formatFixupCount(count: number): string {
  return count === 1 ? "1 fixup" : `${count} fixups`
}

/** The subject of a fixup as shown in a list of fixups: Rust split off its markers and `(prefix)` (`subjectParts`) */
export function getOrphanSubject(orphan: OrphanFixup): { marker: string; text: string } {
  const parts = orphan.subjectParts
  return { marker: (parts?.autosquash ?? "").trim(), text: parts?.rest ?? orphan.subject }
}

/** The marker of a fixup kind, as in the subject */
export function getFixupMarker(kind: FixupKind): string {
  switch (kind) {
    case "Fixup":
      return "fixup!"
    case "Squash":
      return "squash!"
    case "Amend":
      return "amend!"
  }
}

/** What squashing a fixup does to the target's message, for the tooltip of its marker */
export function describeFixupTarget(target: FixupTarget | undefined): string | null {
  if (target === undefined) {
    return null
  }
  return target === null ? "No target in this range" : `Squashes into “${target.targetParts.rest}”`
}
