// The page's words of the lost bindings of an update (`FirstConflict.lostBindings`, `trial_check::lost_bindings`): git
// merges the replay of a commit that removes the import or the private declaration of a name, while the upstream uses
// the name again in the same file, and the file would not compile. It decides nothing.
import type { FirstConflict, UpdateLostBinding } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { splitPath } from "@/lib/update-conflict"

/**
 * "bbbbbbbb removes the import of rethrow, which cccccccc uses at Launch.kt:12": the local commit, what it removes, and
 * where the file after the update uses it, by the upstream commit of that file when it has one, else by the upstream;
 * the file by its name, or by its whole path (`fullPath`, the prompt of an agent)
 */
export function describeLostBinding(lost: UpdateLostBinding, first: FirstConflict, upstream: string, fullPath = false): string {
  const binding = lost.binding === "import" ? "the import of" : "the declaration of"
  const [only, ...others] = first.upstreamCommits.filter(commit => commit.files.includes(lost.file))
  const user = only !== undefined && others.length === 0 ? formatShortHash(only.id) : upstream
  const file = fullPath ? lost.file : splitPath(lost.file).name
  return `${formatShortHash(first.commit.id)} removes ${binding} ${lost.name}, which ${user} uses at ${file}:${lost.useLine}`
}

/** The lost bindings of a conflict, none for a conflict of git */
export function lostBindingsOf(first: FirstConflict): readonly UpdateLostBinding[] {
  return first.lostBindings ?? []
}
