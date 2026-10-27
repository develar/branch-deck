import type { ParkedBranchChange } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import { runRewrite, runUndo } from "@/lib/rewrite"
import type { RewriteSpec } from "@/lib/rewrite"
import { joinNames } from "@/lib/text"
import { getParkedNotes } from "@/stores/branch-sync"
import { branchSubject } from "@/stores/operations"
import { getSelectedRepositoryPath, isSelectedRepository } from "@/stores/repository"

export interface ParkOptions {
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
}

/** Parks `branchName` with `note` (empty for none), or changes the note of a parked one (`parkBranches`) */
export function parkBranch(branchName: string, note: string, options: ParkOptions = {}): Promise<boolean> {
  return parkBranches([branchName], note, options)
}

/**
 * Parks `names` with `note` (empty for none), or changes the notes of parked ones: no rewrite, one commit of
 * `refs/branch-deck/parked` under the repository lock for all of them, then the sync that moves the rows into the Parked
 * card, then the toast with Undo, which puts every line back as it was. A rewrite of its own, so it waits for the other
 * rewrites.
 */
export async function parkBranches(names: readonly string[], note: string, options: ParkOptions = {}): Promise<boolean> {
  // the repository of the click: the park and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const result = await runRewrite(
    parkSpec("park", names, `Parking ${joinNames(names)}`, repositoryPath, options),
    `Cannot park ${joinNames(names)}`,
    wait => commands.parkBranches({ repositoryPath, branches: names.map(branchName => ({ branchName, note })) }, wait),
    data => {
      if (!data.changed) {
        return { sync: false }
      }
      const changed = data.branches.filter(change => change.previousNote !== change.note)
      // a new note of a parked branch is no park
      const [renoted] = changed.length === 1 && changed[0]?.previousNote !== null ? changed : []
      const title = renoted === undefined ? `Parked ${joinNames(changed.map(change => change.branchName))}` : `Changed the park note of ${renoted.branchName}`
      return { sync: true, after: () => showUndoToast(repositoryPath, title, () => restoreQuietly(changed, repositoryPath)) }
    },
  )
  return result !== null
}

/** Unparks `branchName`: it lands as any other branch again (`unparkBranches`) */
export function unparkBranch(branchName: string, options: ParkOptions = {}): Promise<boolean> {
  return unparkBranches([branchName], options)
}

/**
 * Unparks `names` in one commit of the store: they land as any other branch again; the toast's Undo parks them again,
 * each with its note
 */
export async function unparkBranches(names: readonly string[], options: ParkOptions = {}): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const result = await runRewrite(
    parkSpec("unpark", names, `Unparking ${joinNames(names)}`, repositoryPath, options),
    `Cannot unpark ${joinNames(names)}`,
    wait => commands.unparkBranches({ repositoryPath, branchNames: [...names] }, wait),
    data => ({ sync: true, after: () => showUndoToast(repositoryPath, `Unparked ${joinNames(names)}`, () => restoreQuietly(data.branches, repositoryPath)) }),
  )
  return result !== null
}

/**
 * The undo of a park, a new note or an unpark in `repositoryPath`, the repository it was made in: every line of
 * `changes` as it was before, without a toast of its own; the branches that had a line get it back in one park, those
 * that had none and still have one are unparked in one unpark (an unpark since took the line already; the lines are
 * known of the shown repository, so in another one each of them is unparked)
 */
async function restoreQuietly(changes: readonly ParkedBranchChange[], repositoryPath: string): Promise<boolean> {
  const parks = changes.filter(change => change.previousNote !== null)
  const lines = isSelectedRepository(repositoryPath) ? getParkedNotes() : null
  const unparks = changes.filter(change => change.previousNote === null && (lines === null || lines[change.branchName] !== undefined)).map(change => change.branchName)
  if (parks.length > 0) {
    const names = parks.map(change => change.branchName)
    const parked = await runUndo(parkSpec("park", names, `Parking ${joinNames(names)}`, repositoryPath), `Cannot park ${joinNames(names)}`, wait =>
      commands.parkBranches({ repositoryPath, branches: parks.map(change => ({ branchName: change.branchName, note: change.previousNote ?? "" })) }, wait),
    )
    if (!parked) {
      return false
    }
  }
  if (unparks.length > 0) {
    return await runUndo(parkSpec("unpark", unparks, `Unparking ${joinNames(unparks)}`, repositoryPath), `Cannot unpark ${joinNames(unparks)}`, wait =>
      commands.unparkBranches({ repositoryPath, branchNames: unparks }, wait),
    )
  }
  return true
}

function parkSpec(id: "park" | "unpark", names: readonly string[], label: string, repositoryPath: string, options: ParkOptions = {}): RewriteSpec {
  const [only] = names
  return {
    id,
    subject: names.length === 1 && only !== undefined ? branchSubject(only) : `branches:${names.join("\n")}`,
    objects: names,
    label,
    surface: options.surface,
    repositoryPath,
  }
}
