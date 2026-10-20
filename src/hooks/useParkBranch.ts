import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import { runRewrite } from "@/lib/rewrite"
import { branchSubject } from "@/stores/operations"
import { getSelectedProject } from "@/stores/repository"

export interface ParkOptions {
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
}

/**
 * Parks `branchName` with `note` (empty for none), or changes the note of a parked one: no rewrite, one line of
 * `refs/branch-deck/parked` under the repository lock, then the sync that moves the row into the Parked card, then the
 * toast with Undo, which puts the line back as it was. A rewrite of its own, so it waits for the other rewrites.
 */
export async function parkBranch(branchName: string, note: string, options: ParkOptions = {}): Promise<boolean> {
  const result = await runRewrite(
    { id: "park", subject: branchSubject(branchName), objects: [branchName], label: `Parking ${branchName}`, surface: options.surface },
    `Cannot park ${branchName}`,
    () => commands.parkBranch({ repositoryPath: repositoryPath(), branchName, note }),
    data => {
      if (!data.changed) {
        return { sync: false }
      }
      const previous = data.previousNote
      return {
        sync: true,
        after: () =>
          showUndoToast(previous === null ? `Parked ${branchName}` : `Changed the park note of ${branchName}`, () =>
            previous === null ? unparkQuietly(branchName) : parkQuietly(branchName, previous),
          ),
      }
    },
  )
  return result !== null
}

/** Unparks `branchName`: it lands as any other branch again; the toast's Undo parks it again with its note */
export async function unparkBranch(branchName: string, options: ParkOptions = {}): Promise<boolean> {
  const result = await runRewrite(
    { id: "unpark", subject: branchSubject(branchName), objects: [branchName], label: `Unparking ${branchName}`, surface: options.surface },
    `Cannot unpark ${branchName}`,
    () => commands.unparkBranch({ repositoryPath: repositoryPath(), branchName }),
    data => {
      const previous = data.previousNote ?? ""
      return { sync: true, after: () => showUndoToast(`Unparked ${branchName}`, () => parkQuietly(branchName, previous)) }
    },
  )
  return result !== null
}

/** The undo of an unpark or of a new note: the line as it was, without a toast of its own */
async function parkQuietly(branchName: string, note: string): Promise<void> {
  await runRewrite(
    { id: "park", subject: branchSubject(branchName), objects: [branchName], label: `Parking ${branchName}` },
    `Cannot park ${branchName}`,
    () => commands.parkBranch({ repositoryPath: repositoryPath(), branchName, note }),
    () => ({ sync: true }),
  )
}

/** The undo of a park, without a toast of its own */
async function unparkQuietly(branchName: string): Promise<void> {
  await runRewrite(
    { id: "unpark", subject: branchSubject(branchName), objects: [branchName], label: `Unparking ${branchName}` },
    `Cannot unpark ${branchName}`,
    () => commands.unparkBranch({ repositoryPath: repositoryPath(), branchName }),
    () => ({ sync: true }),
  )
}

function repositoryPath(): string {
  return getSelectedProject()?.path ?? ""
}
