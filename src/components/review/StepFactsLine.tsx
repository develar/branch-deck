import { Fragment } from "react"
import { BreakableCode, CodeGlue } from "@/components/shared/CodeText"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { FactList, StepFacts } from "@/presenters/review-story"

/**
 * The facts of a step, muted (spec/branch/review-tour.spec.md, "In the Review Window"): the tests that reach its code,
 * "New API: Cache and Reader" and "New link between modules: Reader.kt → Cache.kt", each name showing its place in the
 * diffs (a declaration, the first use of a link), its file selected, a list the contents cut ending with the count of
 * the rest ("Cache, Reader, Store and 16 more"); of a step none of whose code lands, the commit that took it out, which
 * shows its step; its units the section of another step shows, which shows that step; nothing without a fact. The
 * contents set them in one line, " · " apart; an opening (`lines`) each fact on a line of its own, its tests first, a
 * line each ("New or changed tests reach allowed"), every name in the code font
 */
export function StepFactsLine({ facts, testId, lines = false }: { facts: StepFacts; testId: string; lines?: boolean }) {
  const parts = [
    ...facts.tests.map((fact, index) => <FactPlaces key={`tests:${fact.key}:${index}`} title={fact.words} list={fact.names} testId="review-step-tested" kind={fact.key} />),
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
  if (parts.length === 0) {
    return null
  }
  if (lines) {
    return (
      <ul className="flex flex-col gap-1 text-xs text-muted-foreground" data-testid={testId}>
        {parts.map(part => (
          <li key={part.key} data-testid="review-step-fact">
            {part}
          </li>
        ))}
      </ul>
    )
  }
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

/**
 * The names of a fact, a list in a sentence as `joinNames` writes one: "a, b and c", "a, b, c and 16 more"; each name in
 * the code font, breakable at its dots, its comma on its line (`CodeGlue`)
 */
function FactPlaces({ title, list: { places, more }, testId, kind }: { title: string; list: FactList; testId: string; kind?: string }) {
  const navigator = useSelectReviewFile()
  return (
    <span data-kind={kind}>
      {title}
      {places.map((fact, index) => {
        const last = index === places.length - 1
        // "a, b and c": a comma after each name but the last two; "a, b, c and 16 more": after each but the last
        const comma = !last && !(more === 0 && index === places.length - 2)
        return (
          <Fragment key={fact.key}>
            {index > 0 && last && more === 0 ? " and " : " "}
            <CodeGlue after={comma ? "," : ""}>
              <button
                type="button"
                // a declaration is code, as the story sets its names; a link between modules names its files
                className="rounded-sm font-mono outline-none hover:text-foreground hover:underline focus-visible:bd-focus disabled:no-underline"
                disabled={navigator === null}
                data-testid={testId}
                data-path={fact.place.path}
                onClick={() => navigator?.show(fact.place)}
              >
                <BreakableCode text={fact.label} />
              </button>
            </CodeGlue>
          </Fragment>
        )
      })}
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

/**
 * The warning of a step no test reaches and no scenario proves: "No test reaches paint", "… within 3 calls" when the walk
 * stopped with callers left, each name at its place in the code font
 */
export function UnprovenFact({ unproven, testId }: { unproven: NonNullable<StepFacts["unproven"]>; testId: string }) {
  return (
    <span className="text-xs text-warning" data-testid={testId}>
      <FactPlaces title="No test reaches" list={unproven.names} testId="review-step-unreached" />
      {unproven.within !== null && ` within ${unproven.within} calls`}
    </span>
  )
}
