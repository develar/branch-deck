import { useEffect, useEffectEvent } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { ExternalLinkIcon, SparklesIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { ChoiceChip, softPrimaryClassName } from "@/components/shared/ChoiceChip"
import { InlineCode } from "@/components/shared/InlineCode"
import { useAIToggle } from "@/hooks/useAIToggle"
import { useBranchSuggestions } from "@/hooks/useBranchSuggestions"
import type { SuggestionHint } from "@/hooks/useBranchSuggestions"
import type { Commit } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { aiConsentNote } from "@/stores/settings/aiSettings"

export interface BranchNameSuggestionsProps {
  repositoryPath: string
  commits: readonly Commit[]
  isActive: boolean
  /** The name in the input: its suggestion is the chosen chip */
  selectedName: string
  /** `isAuto`: the first suggestion is auto-populated (the receiver applies it only to an empty, untouched input) */
  onSelect: (name: string, isAuto?: boolean) => void
}

/** Assumed number of suggestions for the loading placeholders */
const EXPECTED_SUGGESTION_COUNT = 2

export const CLAUDE_CODE_SETUP_URL = "https://code.claude.com/docs/en/setup"

/** AI branch name suggestions below the inline branch creator input; on first use, asks whether to enable AI. */
export function BranchNameSuggestions({ repositoryPath, commits, isActive, selectedName, onSelect }: BranchNameSuggestionsProps) {
  const { isInitial, setAIMode } = useAIToggle()
  const { names, isGenerating, hint } = useBranchSuggestions({ repositoryPath, commits, isActive })

  // auto-populate the input with the first suggestion
  const firstName = names.at(0)
  const autoSelect = useEffectEvent((name: string) => onSelect(name, true))
  useEffect(() => {
    if (firstName !== undefined) {
      autoSelect(firstName)
    }
  }, [firstName])

  if (!isInitial && names.length === 0 && !isGenerating && hint === null) {
    return null
  }

  const pendingSlotCount = isGenerating ? Math.max(0, EXPECTED_SUGGESTION_COUNT - names.length) : 0
  return (
    <div className="flex flex-col gap-2" data-testid="branch-name-suggestions">
      <p className="flex items-center gap-1 text-xs text-muted-foreground">
        <SparklesIcon className="size-3" />
        Suggestions:
      </p>

      <div className="flex flex-wrap gap-2">
        {isInitial
          ? (
              <>
                <Button variant="secondary" size="xs" onClick={() => setAIMode("disabled")}>
                  Not now
                </Button>
                <Button variant="ghost" size="xs" className={softPrimaryClassName} onClick={() => setAIMode("claude")}>
                  Enable AI
                </Button>
                <span className="self-center text-xs text-muted-foreground" data-testid="ai-consent-note">
                  {aiConsentNote}
                </span>
              </>
            )
          : (
              <>
                {names.map(name => (
                  <ChoiceChip key={name} chosen={name === selectedName} onClick={() => onSelect(name)}>
                    {name}
                  </ChoiceChip>
                ))}
                {Array.from({ length: pendingSlotCount }, (_, index) => (
                  <Button key={`loading-${index}`} variant="secondary" size="xs" disabled>
                    <Spinner data-icon="inline-start" />
                    Generating…
                  </Button>
                ))}
                {!isGenerating && hint !== null && <SuggestionHintNote hint={hint} />}
              </>
            )}
      </div>
    </div>
  )
}

/** What the user has to do for suggestions; the next request (another selection, the form again) asks again */
function SuggestionHintNote({ hint }: { hint: SuggestionHint }) {
  if (hint === "notLoggedIn") {
    return (
      <span className="self-center text-xs text-muted-foreground" data-testid="ai-suggestion-hint">
        {"Log in to Claude Code to get suggestions: run "}
        <InlineCode>claude</InlineCode>
        {", then "}
        <InlineCode>/login</InlineCode>
      </span>
    )
  }
  return (
    <>
      <span className="self-center text-xs text-muted-foreground" data-testid="ai-suggestion-hint">
        Install Claude Code to get suggestions
      </span>
      <Button
        variant="ghost"
        size="xs"
        className={softPrimaryClassName}
        onClick={() => void openUrl(CLAUDE_CODE_SETUP_URL).catch((error: unknown) => notifyError("Cannot open the Claude Code setup", error))}
      >
        <ExternalLinkIcon data-icon="inline-start" />
        Install Claude Code
      </Button>
    </>
  )
}
