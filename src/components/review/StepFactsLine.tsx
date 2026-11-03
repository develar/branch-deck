import { Fragment } from "react"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { FactList, StepFacts } from "@/presenters/review-story"

/**
 * The facts of a step in one muted line (spec/branch/review-tour.spec.md, "In the Review Window"): the tests that reach
 * its code, "New API: Cache and Reader" and "New link between modules: Reader.kt → Cache.kt", each name showing its
 * place in the diffs (a declaration, the first use of a link), its file selected, a list the contents cut ending with
 * the count of the rest ("Cache, Reader, Store and 16 more"); of a step none of whose code lands, the commit that took
 * it out, which shows its step; its units the section of another step shows, which shows that step; nothing without a
 * fact
 */
export function StepFactsLine({ facts, testId }: { facts: StepFacts; testId: string }) {
  if (facts.line === null && facts.newApi.places.length === 0 && facts.moduleLinks.places.length === 0 && facts.takenOutBy === null && facts.shownElsewhere.length === 0) {
    return null
  }
  const parts = [
    ...(facts.line === null ? [] : [<span key="line">{facts.line}</span>]),
    ...(facts.takenOutBy === null ? [] : [<TakenOutBy key="taken" commit={facts.takenOutBy} />]),
    ...facts.shownElsewhere.map(shown => (
      <StepLink key={`shown:${shown.key}`} before={`${shown.words} `} words={shown.step} step={shown.stepId} title={null} testId="review-step-shown-elsewhere" />
    )),
    ...(facts.newApi.places.length === 0 ? [] : [<FactPlaces key="api" title="New API:" list={facts.newApi} testId="review-step-new-api" />]),
    ...(facts.moduleLinks.places.length === 0
      ? []
      : [
          <FactPlaces
            key="links"
            title={facts.moduleLinks.places.length + facts.moduleLinks.more === 1 ? "New link between modules:" : "New links between modules:"}
            list={facts.moduleLinks}
            testId="review-step-module-link"
          />,
        ]),
  ]
  return (
    <span className="text-xs text-muted-foreground" data-testid={testId}>
      {parts.map((part, index) => (
        <Fragment key={part.key}>
          {index > 0 && " · "}
          {part}
        </Fragment>
      ))}
    </span>
  )
}

/** The names of a fact, a list in a sentence as `joinNames` writes one: "a, b and c", "a, b, c and 16 more" */
function FactPlaces({ title, list: { places, more }, testId }: { title: string; list: FactList; testId: string }) {
  const navigator = useSelectReviewFile()
  return (
    <span>
      {title}
      {places.map((fact, index) => (
        <Fragment key={fact.key}>
          {index === 0 ? " " : index === places.length - 1 && more === 0 ? " and " : ", "}
          <button
            type="button"
            // a declaration is code, as the story sets its names; a link between modules names its files
            className="rounded-sm font-mono outline-none hover:text-foreground hover:underline focus-visible:bd-focus disabled:no-underline"
            disabled={navigator === null}
            data-testid={testId}
            data-path={fact.place.path}
            onClick={() => navigator?.show(fact.place)}
          >
            {fact.label}
          </button>
        </Fragment>
      ))}
      {more > 0 && ` and ${more} more`}
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
        className="rounded-sm text-foreground underline decoration-muted-foreground/60 underline-offset-2 outline-none hover:decoration-foreground focus-visible:bd-focus disabled:text-muted-foreground disabled:no-underline"
        data-testid={testId}
        onClick={show}
      >
        {words}
      </button>
      {after}
    </span>
  )
}
