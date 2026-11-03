// The page's words of a drop of commits (`RewriteIntent::Drop`, spec/commit/drop.spec.md): what its toast adds to
// "Dropped N commits". Rust decides what the drop did; these only word it.
import type { DropCommitsResult } from "@/lib/bindings"

/** What the toast of a drop says below its title: files that did not change, branches left without a commit; null for neither */
export function describeDropped(result: Pick<DropCommitsResult, "filesChanged" | "emptiedBranches">): string | null {
  const parts: string[] = []
  if (!result.filesChanged) {
    parts.push("Your files did not change: the commits cancel each other")
  }
  const emptied = result.emptiedBranches
  if (emptied.length === 1) {
    parts.push(`${emptied[0]} has no commits left`)
  } else if (emptied.length > 1) {
    parts.push(`${emptied.slice(0, -1).join(", ")} and ${emptied.at(-1)} have no commits left`)
  }
  return parts.length === 0 ? null : parts.join(". ")
}
