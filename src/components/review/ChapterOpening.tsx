import { ChevronRightIcon } from "lucide-react"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { StepFactsLine } from "@/components/review/StepFactsLine"
import { StoryBody } from "@/components/review/StoryBody"
import type { OpeningView } from "@/presenters/review-story"

/**
 * The opening of a step on the canvas above the card of its first section (spec/branch/review-tour.spec.md, "In the
 * Review Window"): "Step 2" and the full title, which wraps, a click on them showing the step at the top; the story of
 * the step, the body of the commit that starts it (of a concern of one commit, what joins its parts and the parts of
 * the body told here); the commits that follow it up, each a disclosure of its body, those of no story; the facts of
 * the step in one line, the warning of a step no test reaches; and the group the section starts. As tall as its words:
 * the diffs measure it in place
 */
export function ChapterOpening({ opening, group, onShowStep }: { opening: OpeningView; group: string | null; onShowStep: ((step: string) => void) | undefined }) {
  const { facts } = opening
  return (
    <div className="flex min-w-0 flex-col gap-3 px-1 pt-5 pb-3" data-testid="review-opening" data-step={opening.id}>
      <button
        type="button"
        data-testid="review-concern-heading"
        data-step={opening.id}
        className="flex max-w-prose min-w-0 flex-col items-start gap-0.5 self-start rounded-sm text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30 [&:hover>span:last-child]:underline"
        onClick={() => onShowStep?.(opening.id)}
      >
        <span className="text-xs font-medium text-muted-foreground" data-testid="review-opening-number">
          {opening.number}
        </span>
        {/* the name of the button and its text: "Step 2 · The open keeps a stored layout" */}
        <span className="sr-only"> · </span>
        <span className="text-base font-semibold text-foreground underline-offset-4" data-testid="review-opening-title">
          {opening.title}
        </span>
      </button>
      {opening.evidence !== null && (
        <p className="text-xs text-muted-foreground" data-testid="review-opening-evidence">
          {opening.evidence}
        </p>
      )}
      {opening.story !== null && <StoryBody story={opening.story} />}
      {opening.followUps.length > 0 && (
        <div className="flex flex-col gap-1.5">
          {opening.followUps.map(commit =>
            commit.story === null ? (
              <span key={commit.id} className="text-sm" data-testid="review-opening-commit">{`Follows up: ${commit.title}`}</span>
            ) : (
              <Collapsible key={commit.id} className="flex flex-col gap-2" data-testid="review-opening-follow-up">
                <CollapsibleTrigger className="group flex max-w-prose min-w-0 items-baseline gap-1 self-start rounded-sm text-left text-sm outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30">
                  <ChevronRightIcon aria-hidden className="size-3.5 shrink-0 self-center text-muted-foreground transition-transform group-data-[panel-open]:rotate-90" />
                  <span className="shrink-0 text-muted-foreground">Follows up:</span>
                  <span className="min-w-0 font-medium" data-testid="review-opening-commit">
                    {commit.title}
                  </span>
                </CollapsibleTrigger>
                <CollapsibleContent className="pl-4.5">
                  <StoryBody story={commit.story} />
                </CollapsibleContent>
              </Collapsible>
            ),
          )}
        </div>
      )}
      {opening.withoutStory.length > 0 && (
        <ul className="flex flex-col gap-0.5 text-sm" aria-label="Commits without a story">
          {opening.withoutStory.map(commit => (
            <li key={commit.id} data-testid="review-opening-commit">
              <span className="text-muted-foreground">Without a story: </span>
              {commit.title}
            </li>
          ))}
        </ul>
      )}
      {facts.unproven !== null && (
        <span className="text-xs text-warning" data-testid="review-opening-unproven">
          {facts.unproven}
        </span>
      )}
      <StepFactsLine facts={facts} testId="review-opening-facts" />
      {group !== null && (
        <span data-testid="review-section-heading" className="text-xs text-muted-foreground">
          {group}
        </span>
      )}
    </div>
  )
}
