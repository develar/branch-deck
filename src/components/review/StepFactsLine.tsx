import { Fragment } from "react"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { FactPlace, StepFacts } from "@/presenters/review-story"

/**
 * The facts of a step in one muted line (spec/branch/review-tour.spec.md, "In the Review Window"): the tests that reach
 * its code, "New API: Cache, Reader" and "New link between modules: Reader.kt → Cache.kt", each name showing its place
 * in the diffs (a declaration, the first use of a link), its file selected; of a step none of whose code lands, the
 * commit that took it out, which shows its step; its units the section of another step shows, which shows that step;
 * nothing without a fact
 */
export function StepFactsLine({ facts, testId }: { facts: StepFacts; testId: string }) {
  if (facts.line === null && facts.newApi.length === 0 && facts.moduleLinks.length === 0 && facts.takenOutBy === null && facts.shownElsewhere.length === 0) {
    return null
  }
  const parts = [
    ...(facts.line === null ? [] : [<span key="line">{facts.line}</span>]),
    ...(facts.takenOutBy === null ? [] : [<TakenOutBy key="taken" commit={facts.takenOutBy} />]),
    ...facts.shownElsewhere.map(shown => (
      <StepLink key={`shown:${shown.key}`} before={`${shown.words} `} words={shown.step} step={shown.stepId} title={null} testId="review-step-shown-elsewhere" />
    )),
    ...(facts.newApi.length === 0 ? [] : [<FactPlaces key="api" title="New API:" places={facts.newApi} testId="review-step-new-api" />]),
    ...(facts.moduleLinks.length === 0 ? [] : [<FactPlaces key="links" title="New link between modules:" places={facts.moduleLinks} testId="review-step-module-link" />]),
  ]
  return (
    <span className="max-w-prose text-xs text-muted-foreground" data-testid={testId}>
      {parts.map((part, index) => (
        <Fragment key={part.key}>
          {index > 0 && " · "}
          {part}
        </Fragment>
      ))}
    </span>
  )
}

function FactPlaces({ title, places, testId }: { title: string; places: readonly FactPlace[]; testId: string }) {
  const navigator = useSelectReviewFile()
  return (
    <span>
      {title}
      {places.map((fact, index) => (
        <Fragment key={fact.key}>
          {index > 0 ? ", " : " "}
          <button
            type="button"
            // a declaration is code, as the story sets its names; a link between modules names its files
            className="rounded-sm font-mono outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/30 disabled:no-underline"
            disabled={navigator === null}
            data-testid={testId}
            data-path={fact.place.path}
            onClick={() => navigator?.show(fact.place)}
          >
            {fact.label}
          </button>
        </Fragment>
      ))}
    </span>
  )
}

/**
 * "Nothing of it lands: step 5 took it out", "step 5" showing that step, the title of the commit that took the code out
 * in its hover
 */
function TakenOutBy({ commit }: { commit: NonNullable<StepFacts["takenOutBy"]> }) {
  return <StepLink before="Nothing of it lands: " words={commit.words} step={commit.step} title={commit.title} after=" took it out" testId="review-step-taken-out-by" />
}

/** Words of the line that name another step ("step 5"), a button of the text that shows that step, as the other names of the line */
function StepLink({
  before,
  words,
  step,
  title,
  after = "",
  testId,
}: {
  before: string
  words: string
  step: string | null
  title: string | null
  after?: string
  testId: string
}) {
  const navigator = useSelectReviewFile()
  const enabled = navigator !== null && step !== null
  const show = () => {
    if (enabled) {
      navigator.showStep(step)
    }
  }
  return (
    <span>
      {before}
      <button
        type="button"
        disabled={!enabled}
        title={title ?? undefined}
        className="rounded-sm text-foreground underline decoration-muted-foreground/60 underline-offset-2 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/30 disabled:text-muted-foreground disabled:no-underline"
        data-testid={testId}
        onClick={show}
      >
        {words}
      </button>
      {after}
    </span>
  )
}
