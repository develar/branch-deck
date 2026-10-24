/**
 * The view of a session after a patch, as `repository_model::patch::apply_patch` makes it in Rust: the parts the patch
 * names replace theirs, the keyed parts (branches, stacks, the integration of the archived branches) only for the keys
 * it names. The page's store (`src/model/store.ts`) applies the patches of its session with it, the fake sessions of
 * the store tests (`src/test-support/fake-sessions.ts`) keep their views with it
 */
import type { BranchChange, BranchView, TableView, ViewPatch } from "@/lib/bindings"
import { withoutKeys } from "@/lib/record"

/** The view after `patch`; the caller checked that `view` is at its base */
export function applyViewPatch(view: TableView, patch: ViewPatch): TableView {
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
