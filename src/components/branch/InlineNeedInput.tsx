import { useState } from "react"
import { GitBranchIcon, ListOrderedIcon } from "lucide-react"
import { ChoiceChip } from "@/components/shared/ChoiceChip"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import { declareNeed } from "@/hooks/useDeclareNeed"
import { getDeclaredNeeds, getMainBranchOrder } from "@/stores/branch-sync"
import { closeInline, inlineSurface, withRowProgress } from "@/stores/inline-row-action"

/** The most branches the form offers as chips */
const MAX_NEED_CHIPS = 6

/**
 * Add Need… (openspec/specs/branch/declared-needs/spec.md): what lands before the branch, typed or picked from the other branches
 * of the table; a branch of a companion as `companion:branch`. Add Need writes it, the row names it; a refusal of the
 * core (an unknown branch, a need of itself) keeps the form with the typing
 */
export function InlineNeedInput({ branchName }: { branchName: string }) {
  const [needed, setNeeded] = useState("")
  // read once as the form opens: the branches it may name and the needs already declared
  const [chips] = useState(() => {
    const declared = new Set(getDeclaredNeeds(branchName))
    return getMainBranchOrder()
      .filter(name => name !== branchName && !declared.has(name))
      .slice(0, MAX_NEED_CHIPS)
  })
  const typed = needed.trim()
  return (
    <InlineInputDialog
      value={needed}
      onValueChange={setNeeded}
      placeholder="A branch of this repository, or companion:branch"
      canSubmit={typed !== "" && !/\s/.test(typed)}
      title={`What lands before ${branchName}`}
      submitText="Add Need"
      dataTestId="inline-need-input"
      leadingIcon={<ListOrderedIcon className="size-4 text-muted-foreground" />}
      helpText={`No Safe Push, Dry Run or land of ${branchName} starts until that branch is in master.`}
      afterControls={
        chips.length > 0 && (
          <div className="flex flex-wrap gap-2" data-testid="need-chips">
            {chips.map(name => (
              <ChoiceChip key={name} chosen={name === typed} onClick={() => setNeeded(name)}>
                <GitBranchIcon data-icon="inline-start" />
                {name}
              </ChoiceChip>
            ))}
          </div>
        )
      }
      onSubmit={() => {
        void withRowProgress(branchName, () => declareNeed(branchName, [typed], { surface: inlineSurface(branchName) }), { keepFormOnFailure: true })
      }}
      onCancel={closeInline}
    />
  )
}
