// TODO(Phase 8): placeholder; replaced by the full port of BranchNameSuggestions.vue
//  (initial "Enable AI" prompt, streamed suggestions with loading progress, auto-population via `onSelect(name, true)`).
import type { Commit } from "@/lib/bindings"

export interface BranchNameSuggestionsProps {
  repositoryPath: string
  branchPrefix: string
  commits: readonly Commit[]
  isActive: boolean
  /** `isAuto`: the first suggestion is auto-populated (the receiver applies it only to an empty, untouched input) */
  onSelect: (name: string, isAuto?: boolean) => void
}

/** AI branch name suggestions below the inline branch creator input. */
export function BranchNameSuggestions(_props: BranchNameSuggestionsProps) {
  return <div className="flex flex-col gap-2 empty:hidden" data-testid="branch-name-suggestions" />
}
