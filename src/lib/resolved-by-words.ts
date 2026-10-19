import type { WordResolutionSummary } from "@/lib/bindings"

/** "Resolved 3 conflicts by words": the notice on the row of a commit whose copy took the resolve by words */
export function resolvedNotice(resolved: WordResolutionSummary): string {
  return resolved.conflicts === 1 ? "Resolved 1 conflict by words" : `Resolved ${resolved.conflicts} conflicts by words`
}
