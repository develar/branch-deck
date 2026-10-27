// The delete of a branch Branch Deck keeps beside the virtual ones (Delete Archived Branch…, Delete Unapplied Branch…):
// the rewrite `deleteArchived` or `deleteUnapplied` of the engine (`operations::rewrite`). It holds only the ref it
// drops, so it runs beside a rewrite and a sync, and a push of that branch waits for it; the session removes its row by
// a patch, no sync follows (spec/branch/branch-row.spec.md, spec/branch/unapply.spec.md).
import type { RewriteIntent } from "@/lib/bindings"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { showToast } from "@/lib/toast-message"
import { inlineSurface } from "@/stores/inline-row-action"
import { branchSubject } from "@/stores/operations"
import { getSelectedRepositoryPath } from "@/stores/repository"

type DeleteIntent = Extract<RewriteIntent, { rewrite: "deleteArchived" | "deleteUnapplied" }>

/** The kind of the branch each delete deletes, in its words, and the id of its menu item */
const DELETES = {
  deleteArchived: { kind: "archived", id: "delete-archived" },
  deleteUnapplied: { kind: "unapplied", id: "delete-unapplied" },
} as const

/**
 * Deletes the kept branch `name` (its full name; `shown` is how its row names it) from the inline form of its row, in
 * the repository the main window shows: the form shows the delete while the session runs it, a toast says it once the
 * row is gone ("Deleted the archived branch feature-partial"). Resolves to false, after an error toast ("Cannot delete
 * the unapplied branch docs"), when nothing was deleted
 */
export async function deleteKeptBranch(rewrite: DeleteIntent["rewrite"], name: string, shown: string): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const { kind, id } = DELETES[rewrite]
  const branch = `the ${kind} branch ${shown}`
  const spec = { id, subject: branchSubject(name), objects: [name], label: `Deleting ${shown}`, surface: inlineSurface(name), repositoryPath }
  const done = await runRewriteIntent(spec, witnessed<DeleteIntent>(repositoryPath, { rewrite, branchName: name }), `Cannot delete ${branch}`)
  if (done === null) {
    return false
  }
  showToast({ title: `Deleted ${branch}` })
  return true
}
