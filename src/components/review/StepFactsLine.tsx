import { Fragment } from "react"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { FactPlace, StepFacts } from "@/presenters/review-story"

/**
 * The facts of a step in one muted line (spec/branch/review-tour.spec.md, "In the Review Window"): the tests that reach
 * its code, "New API: Cache, Reader" and "New link between modules: Reader.kt → Cache.kt", each name showing its place
 * in the diffs (a declaration, the first use of a link), its file selected; of a step none of whose code lands, the
 * commit that took it out, which shows its step; nothing without a fact
 */
export function StepFactsLine({ facts, testId }: { facts: StepFacts; testId: string }) {
  if (facts.line === null && facts.newApi.length === 0 && facts.moduleLinks.length === 0 && facts.takenOutBy === null) {
    return null
  }
  const parts = [
    ...(facts.line === null ? [] : [<span key="line">{facts.line}</span>]),
    ...(facts.takenOutBy === null ? [] : [<TakenOutBy key="taken" commit={facts.takenOutBy} />]),
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
            className="rounded-sm outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/30 disabled:no-underline"
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
 * in its hover: a button of the text, as the other names of the line
 */
function TakenOutBy({ commit }: { commit: NonNullable<StepFacts["takenOutBy"]> }) {
  const navigator = useSelectReviewFile()
  const step = commit.step
  const enabled = navigator !== null && step !== null
  const show = () => {
    if (enabled) {
      navigator.showStep(step)
    }
  }
  return (
    <span>
      {"Nothing of it lands: "}
      <button
        type="button"
        disabled={!enabled}
        title={commit.title}
        className="rounded-sm text-foreground underline decoration-muted-foreground/60 underline-offset-2 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/30 disabled:text-muted-foreground disabled:no-underline"
        data-testid="review-step-taken-out-by"
        onClick={show}
      >
        {commit.words}
      </button>
      {" took it out"}
    </span>
  )
}
