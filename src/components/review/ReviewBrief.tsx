import { ChevronRightIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { LinkedText } from "@/components/commit/LinkedText"
import { SectionLabel } from "@/components/shared/SectionLabel"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { ReviewPlan } from "@/lib/bindings"
import { fileTop } from "@/lib/review-place"
import type { BriefStep } from "@/presenters/review-steps"
import { presentBrief } from "@/presenters/review-steps"

/**
 * The brief of a review with its plan (spec/branch/review-tour.spec.md, "In the Review Window"), before any diff: the
 * issue the commits name, what changes in the words of the specs and the docs the change adds, and the steps of its
 * story, each with the why its commit tells, its rounds and the tests that name its code; a step opens its first
 * section in the diffs. Rust tells and orders; this words what it sent.
 */
export function ReviewBrief({ plan }: { plan: Pick<ReviewPlan, "steps" | "concerns" | "brief"> }) {
  const brief = presentBrief(plan)
  const navigator = useSelectReviewFile()
  if (brief.steps.length === 0 && brief.contract.length === 0) {
    return null
  }
  return (
    <div className="flex flex-col gap-2" data-testid="review-brief">
      {brief.issue !== null && <LinkedText text={brief.issue} className="font-medium" linkClassName="text-foreground underline-offset-2 hover:underline" />}
      {brief.contract.length > 0 && (
        <div className="flex flex-col gap-1" data-testid="review-brief-contract">
          <SectionLabel title="What Changes" />
          <ul className="flex flex-col gap-1.5">
            {brief.contract.map(entry => (
              <li key={entry.key} className="flex min-w-0 flex-col gap-0.5" data-testid="review-brief-contract-entry" data-path={entry.path}>
                <span className="flex min-w-0 items-center gap-2 text-xs">
                  <Badge variant="secondary">{entry.kind}</Badge>
                  <button
                    type="button"
                    className="truncate rounded-sm text-muted-foreground outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30 disabled:no-underline"
                    title={entry.path}
                    disabled={navigator === null}
                    onClick={() => navigator?.show(fileTop(entry.path))}
                  >
                    {entry.heading === null ? entry.path : `${entry.path} · ${entry.heading}`}
                  </button>
                  {entry.provenBy !== null && <span className="ml-auto shrink-0 text-muted-foreground">{entry.provenBy}</span>}
                </span>
                {entry.paragraphs.map(paragraph => (
                  <LinkedText key={paragraph} text={paragraph} detectCode className="pl-3" />
                ))}
                {entry.removed.map(paragraph => (
                  <LinkedText key={paragraph} text={paragraph} detectCode className="pl-3 text-muted-foreground line-through" />
                ))}
              </li>
            ))}
          </ul>
        </div>
      )}
      {brief.steps.length > 0 && (
        <div className="flex flex-col gap-1" data-testid="review-brief-steps">
          <SectionLabel title="Steps" count={brief.steps.length}>
            {brief.source !== null && <span className="text-xs text-muted-foreground">{brief.source}</span>}
          </SectionLabel>
          <ol className="flex flex-col gap-2">
            {brief.steps.map(step => (
              <BriefStepItem key={step.id} step={step} onShow={navigator === null ? null : () => navigator.showStep(step.id)} />
            ))}
          </ol>
        </div>
      )}
    </div>
  )
}

/** A step of the brief: its number and title, which open it; its why, the rest of its body folded; its rounds and tests */
function BriefStepItem({ step, onShow }: { step: BriefStep; onShow: (() => void) | null }) {
  return (
    <li className="flex min-w-0 flex-col gap-0.5" data-testid="review-brief-step" data-step={step.id}>
      <span className="flex min-w-0 items-baseline gap-2">
        <span className="shrink-0 text-muted-foreground tabular-nums">{step.number}</span>
        <button
          type="button"
          className="min-w-0 truncate rounded-sm text-left font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30 disabled:no-underline"
          title={step.title}
          disabled={onShow === null}
          data-testid="review-brief-step-title"
          onClick={() => onShow?.()}
        >
          {step.title}
        </button>
        {step.unproven !== null && (
          <span className="ml-auto shrink-0 text-xs text-warning" data-testid="review-brief-step-unproven">
            {step.unproven}
          </span>
        )}
      </span>
      <div className="flex flex-col gap-0.5 pl-5">
        {step.why !== null && <LinkedText text={step.why} detectCode className="text-muted-foreground" />}
        {step.more.length > 0 && (
          <Collapsible className="flex flex-col gap-0.5">
            <CollapsibleTrigger className="group flex items-center gap-1 self-start rounded-sm text-xs font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30">
              <ChevronRightIcon className="size-3.5 shrink-0 transition-transform group-data-[panel-open]:rotate-90" />
              More
            </CollapsibleTrigger>
            <CollapsibleContent className="flex flex-col gap-1">
              {step.more.map(paragraph => (
                <LinkedText key={paragraph} text={paragraph} detectCode className="text-muted-foreground" />
              ))}
            </CollapsibleContent>
          </Collapsible>
        )}
        {step.landsNothing && <span className="text-xs text-muted-foreground">Nothing of it lands: a later commit took it out</span>}
        {[...step.rounds, step.named, step.scenarios, step.unnamed]
          .filter(line => line !== null)
          .map(line => (
            <span key={line} className="truncate text-xs text-muted-foreground" title={line} data-testid="review-brief-step-fact">
              {line}
            </span>
          ))}
      </div>
    </li>
  )
}
