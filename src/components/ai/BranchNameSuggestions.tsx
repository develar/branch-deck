import { useEffect, useEffectEvent } from "react"
import { SparklesIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useAIToggle } from "@/hooks/useAIToggle"
import { useBranchSuggestions } from "@/hooks/useBranchSuggestions"
import type { Commit } from "@/lib/bindings"

export interface BranchNameSuggestionsProps {
  repositoryPath: string
  branchPrefix: string
  commits: readonly Commit[]
  isActive: boolean
  /** `isAuto`: the first suggestion is auto-populated (the receiver applies it only to an empty, untouched input) */
  onSelect: (name: string, isAuto?: boolean) => void
}

/** Soft primary button: tinted primary background, primary text */
const primarySoftClassName = "bg-primary/10 text-primary hover:bg-primary/20 hover:text-primary"

/** Assumed number of suggestions for the loading placeholders */
const EXPECTED_SUGGESTION_COUNT = 2

/** AI branch name suggestions below the inline branch creator input; on first use, asks whether to enable AI. */
export function BranchNameSuggestions({ repositoryPath, branchPrefix, commits, isActive, onSelect }: BranchNameSuggestionsProps) {
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
          <div className="text-xs text-muted-foreground">
            {Math.round(loadingProgress)}
            %
          </div>
        )}
      </div>

      <div className="flex flex-wrap gap-2">
        {showInitialPrompt
          ? (
              <>
                <Button variant="secondary" size="xs" onClick={() => void setAIMode("disabled")}>
                  Not now
                </Button>
                <Button variant="ghost" size="xs" className={primarySoftClassName} onClick={() => void setAIMode("enabled")}>
                  Enable AI
                </Button>
              </>
            )
          : (
              <>
                {suggestions.map((suggestion, index) => (
                  <Button
                    key={suggestion.name}
                    size="xs"
                    variant={index === 0 ? "ghost" : "secondary"}
                    className={index === 0 ? primarySoftClassName : undefined}
                    onClick={() => onSelect(suggestion.name)}
                  >
                    <span>{suggestion.name}</span>
                  </Button>
                ))}
                {Array.from({ length: pendingSlotCount }, (_, index) => (
                  <div
                    key={`loading-${index}`}
                    className="inline-flex animate-pulse items-center justify-center gap-1 rounded-lg border bg-background px-2 py-1 text-xs font-medium"
                  >
                    <SparklesIcon className="size-3 animate-spin text-muted-foreground" />
                    <span className="text-muted-foreground">Generating…</span>
                  </div>
                ))}
              </>
            )}
      </div>
    </div>
  )
}
