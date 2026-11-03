import { Fragment } from "react"
import { cn } from "cn"
import { StepFactsLine, UnprovenFact } from "@/components/review/StepFactsLine"
import { StoryBody, SubjectBefore } from "@/components/review/StoryBody"
import { BreakableCode } from "@/components/shared/CodeText"
import { Disclosure } from "@/components/shared/Disclosure"
import { ReadingColumn } from "@/components/shared/ReadingColumn"
import type { OpeningView } from "@/presenters/review-story"

/**
 * The opening of a step, a card of prose above the card of its first section (spec/branch/review-tour.spec.md, "In the
 * Review Window"): "Step 2" and the full title, which wraps, a click on them showing the step at the top (a title that
 * is the first sentence of the paragraph the step tells only for a screen reader: the paragraph, a step larger, shows
 * it); the story of the step, the body of the commit that starts it (of a concern of one commit and of a part of its
 * message, the parts of the body told here); the
 * other commits of the step, each one row of the same form (a muted "Follows up" or "Without a story", its title; a
 * commit with a story a disclosure of it, at the size of the row); the facts of the step in one muted line, what joins
 * the parts of a concern among them, the warning of a step no test reaches; and the group the section starts. As tall
 * as its words: the diffs measure it in place
 */
export function ChapterOpening({ opening, group, onShowStep }: { opening: OpeningView; group: string | null; onShowStep: ((step: string) => void) | undefined }) {
  const { facts } = opening
  return (
    <ReadingColumn className="gap-3 px-1" data-testid="review-opening" data-step={opening.id}>
      <button
        type="button"
        data-testid="review-concern-heading"
        data-step={opening.id}
        className="flex min-w-0 flex-col items-start gap-0.5 self-start rounded-sm text-left outline-none focus-visible:bd-focus [&:hover>span:last-child]:underline"
        onClick={() => onShowStep?.(opening.id)}
      >
        <span className="text-xs font-medium text-muted-foreground" data-testid="review-opening-number">
          {opening.number}
        </span>
        {/* the name of the button and its text: "Step 2 · The open keeps a stored layout" */}
        <span className="sr-only"> · </span>
        {/* a title that is the first sentence of the paragraph below is read there, a step larger, not twice */}
        <span
          className={cn("text-base font-semibold text-foreground underline-offset-4", opening.code && "font-mono font-medium", opening.titleInBody && "sr-only")}
          data-testid="review-opening-title"
        >
          {opening.code ? <BreakableCode text={opening.title} humps /> : opening.title}
        </span>
      </button>
      {opening.subjectBefore !== null && <SubjectBefore subject={opening.subjectBefore} />}
      {opening.story !== null && <StoryBody story={opening.story} />}
      {opening.commits.length > 0 && (
        <ul className="flex flex-col gap-1.5 text-sm" aria-label="The other commits of the step">
          {opening.commits.map(commit => (
            <li key={commit.id} data-testid={commit.story === null ? "review-opening-commit-row" : "review-opening-follow-up"}>
              {commit.story === null ? (
                // no story to open: no chevron, its title in the place of the title of a disclosure
                <span className="flex min-w-0 items-baseline gap-1 pl-[1.125rem]">
                  <span className="shrink-0 text-muted-foreground">{commit.role}</span>
                  <span className="min-w-0" data-testid="review-opening-commit">
                    {commit.title}
                  </span>
                </span>
              ) : (
                <Disclosure prefix={commit.role} title={<span data-testid="review-opening-commit">{commit.title}</span>}>
                  <StoryBody story={commit.story} size="nested" />
                </Disclosure>
              )}
            </li>
          ))}
        </ul>
      )}
      {facts.unproven !== null && <UnprovenFact unproven={facts.unproven} testId="review-opening-unproven" />}
      {opening.evidence.length > 0 && (
        <p className="text-xs text-muted-foreground" data-testid="review-opening-evidence">
          {opening.evidence.map((part, index) => (
            <Fragment key={part.words}>
              {index > 0 && " · "}
              {part.words}
              {part.names.map((name, at) => (
                <Fragment key={name}>
                  {at > 0 ? ", " : " "}
                  <code className="font-mono">{name}</code>
                </Fragment>
              ))}
            </Fragment>
          ))}
        </p>
      )}
      <StepFactsLine facts={facts} testId="review-opening-facts" lines />
      {group !== null && (
        <span data-testid="review-section-heading" className="text-xs text-muted-foreground">
          {group}
        </span>
      )}
    </ReadingColumn>
  )
}
