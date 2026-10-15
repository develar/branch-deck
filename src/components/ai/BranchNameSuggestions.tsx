import { useEffect, useEffectEvent } from "react"
import { SparklesIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { ChoiceChip, softPrimaryClassName } from "@/components/shared/ChoiceChip"
import { useAIToggle } from "@/hooks/useAIToggle"
import { useBranchSuggestions } from "@/hooks/useBranchSuggestions"
import type { Commit } from "@/lib/bindings"
import { ensureModelStatus, useModelStore } from "@/stores/model"

export interface BranchNameSuggestionsProps {
  repositoryPath: string
  branchPrefix: string
  commits: readonly Commit[]
  isActive: boolean
  /** The name in the input: its suggestion is the chosen chip */
  selectedName: string
  /** `isAuto`: the first suggestion is auto-populated (the receiver applies it only to an empty, untouched input) */
  onSelect: (name: string, isAuto?: boolean) => void
}

/** Assumed number of suggestions for the loading placeholders */
const EXPECTED_SUGGESTION_COUNT = 2

/** AI branch name suggestions below the inline branch creator input; on first use, asks whether to enable AI. */
export function BranchNameSuggestions({ repositoryPath, branchPrefix, commits, isActive, selectedName, onSelect }: BranchNameSuggestionsProps) {
  const { isInitial, setAIMode } = useAIToggle()
  const { suggestions, isGenerating, loadingProgress } = useBranchSuggestions({ repositoryPath, branchPrefix, commits, isActive })

  // auto-populate the input with the first suggestion
  const firstSuggestionName = suggestions.at(0)?.name
  const autoSelect = useEffectEvent((name: string) => onSelect(name, true))
  useEffect(() => {
    if (firstSuggestionName !== undefined) {
      autoSelect(firstSuggestionName)
    }
  }, [firstSuggestionName])

  if (!isInitial && suggestions.length === 0 && !isGenerating) {
    return null
  }

  const showInitialPrompt = isInitial && !isGenerating && suggestions.length === 0
  const pendingSlotCount = isGenerating ? Math.max(0, EXPECTED_SUGGESTION_COUNT - suggestions.length) : 0
  return (
    <div className="flex flex-col gap-2" data-testid="branch-name-suggestions">
      <div className="flex items-center justify-between">
        <p className="flex items-center gap-1 text-xs text-muted-foreground">
          <SparklesIcon className="size-3" />
          Suggestions:
        </p>
        {isGenerating && loadingProgress > 0 && (
          <div className="text-xs text-muted-foreground tabular-nums">{`${Math.round(loadingProgress)}%`}</div>
        )}
      </div>

      <div className="flex flex-wrap gap-2">
        {showInitialPrompt
          ? (
              <>
                <Button variant="secondary" size="xs" onClick={() => void setAIMode("disabled")}>
                  Not now
                </Button>
                <Button variant="ghost" size="xs" className={softPrimaryClassName} onClick={() => void setAIMode("enabled")}>
                  Enable AI
                </Button>
                <ModelDownloadNote />
              </>
            )
          : (
              <>
                {suggestions.map(suggestion => (
                  <ChoiceChip key={suggestion.name} chosen={suggestion.name === selectedName} onClick={() => onSelect(suggestion.name)}>
                    {suggestion.name}
                  </ChoiceChip>
                ))}
                {Array.from({ length: pendingSlotCount }, (_, index) => (
                  <Button key={`loading-${index}`} variant="secondary" size="xs" disabled>
                    <Spinner data-icon="inline-start" />
                    Generating…
                  </Button>
                ))}
              </>
            )}
      </div>
    </div>
  )
}

/** What Enable AI costs: the download, before the user agrees to it */
function ModelDownloadNote() {
  useEffect(() => {
    ensureModelStatus()
  }, [])
  const status = useModelStore(state => state.modelStatus)
  if (status === null || status.available) {
    return null
  }
  return (
    <span className="self-center text-xs text-muted-foreground" data-testid="ai-download-note">
      {`Downloads a ${status.modelSize} model once; it runs on your machine`}
    </span>
  )
}
