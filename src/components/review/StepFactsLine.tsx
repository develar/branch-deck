import { Fragment, useEffect, useRef, useState } from "react"
import { BreakableCode, CodeGlue } from "@/components/shared/CodeText"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { FactList, StepFacts } from "@/presenters/review-story"

/**
 * The facts of a step, muted (openspec/specs/branch/review-tour/spec.md, "In the Review Window"): the files of the tests of the
 * change that reach its code ("Tested by ManagerTest.kt", each its tests and their ways in its hover), "New API: Cache
 * and Reader", "New module view.gui.media: 13 declarations" (their names in the hover), "Opens wider: mediaDuration" and "New link between modules: Reader.kt → Cache.kt", each name showing its
 * place in the diffs (a file of tests at its first test, a declaration, the first use of a link), its file selected, a
 * list the contents cut ending with the count of the rest ("Cache, Reader, Store and 16 more"), a long list of the
 * opening its first eight and "and N more", a button that shows the rest in place; of a step none of whose
 * code lands, the commit that took it out, which shows its step; its units the section of another step shows ("Removes
 * hooks, shown in step 2"), each name at its place and "step 2" showing that step; the steps its tests prove ("Proves
 * step 2"); nothing without a fact. The contents set them in one line, " · " apart; an opening (`lines`) each fact on a
 * line of its own, its tests first; every name in the code font
 */
export function StepFactsLine({ facts, testId, lines = false }: { facts: StepFacts; testId: string; lines?: boolean }) {
  const parts = [
    ...facts.tests.map((fact, index) => <FactPlaces key={`tests:${fact.key}:${index}`} title={fact.words} list={fact.names} testId="review-step-tested" kind={fact.key} />),
    ...(facts.line === null ? [] : [<span key="line">{facts.line}</span>]),
    ...facts.proves.map((proved, index) => (
      <StepLink key={`proves:${proved.key}`} before={index === 0 ? "Proves " : ""} words={proved.step} step={proved.stepId} title={null} testId="review-step-proves" />
    )),
    ...(facts.takenOutBy === null ? [] : [<TakenOutBy key="taken" commit={facts.takenOutBy} />]),
    ...facts.shownElsewhere.map(shown => (
      <span key={`shown:${shown.key}`}>
        <FactPlaces title={shown.verb} list={shown.names} unnamed={shown.tests} testId="review-step-shown-elsewhere-name" />
        <StepLink before=", shown in " words={shown.step} step={shown.stepId} title={null} testId="review-step-shown-elsewhere" />
      </span>
    )),
    ...(facts.newApi.places.length === 0 ? [] : [<FactPlaces key="api" title="New API:" list={facts.newApi} testId="review-step-new-api" />]),
    ...facts.newModules.map(added => (
      <span key={`module:${added.key}`} data-testid="review-step-new-module">
        <FactPlaces title="New module" list={{ places: [added.module], more: 0 }} testId="review-step-new-module-name" />
        {`: ${added.count}`}
      </span>
    )),
    ...(facts.opensWider.places.length === 0 ? [] : [<FactPlaces key="wider" title="Opens wider:" list={facts.opensWider} testId="review-step-opens-wider" />]),
    ...(facts.uses === undefined || facts.uses.places.length === 0 ? [] : [<FactPlaces key="uses" title="Now uses" list={facts.uses} testId="review-step-module-use" />]),
    ...(facts.usedBy === undefined || facts.usedBy.places.length === 0
      ? []
      : [<FactPlaces key="used-by" title="Now used by" list={facts.usedBy} testId="review-step-module-user" />]),
    ...(facts.moduleEdges.places.length === 0
      ? []
      : [
          <FactPlaces
            key="modules"
            title={facts.moduleEdges.places.length + facts.moduleEdges.more === 1 ? "New use between modules:" : "New uses between modules:"}
            list={facts.moduleEdges}
            testId="review-step-module-edge"
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
 * The names of a fact, a list in a sentence as `joinNames` writes one: "a, b and c", "a, b, c and 16 more", "a, b and 2
 * tests" with what it has no name for (`unnamed`); each name in the code font, breakable at its dots, its comma on its
 * line (`CodeGlue`)
 */
function FactPlaces({
  title,
  list: { places, more, rest = [] },
  unnamed = null,
  testId,
  kind,
}: {
  title: string
  list: FactList
  unnamed?: string | null
  testId: string
  kind?: string
}) {
  const navigator = useSelectReviewFile()
  // "and N more" of the opening shows the rest in place, the focus on the first name it showed
  const [open, setOpen] = useState(false)
  const firstRest = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    if (open) {
      firstRest.current?.focus()
    }
  }, [open])
  const folded = !open && rest.length > 0
  const shown = folded ? places : [...places, ...rest]
  // the words after the names: the count of the rest, else what has no name
  const tail = more > 0 && (folded || rest.length === 0) ? `${more} more` : unnamed
  return (
    <span data-kind={kind}>
      {title}
      {shown.map((fact, index) => {
        const last = index === shown.length - 1
        // "a, b and c": a comma after each name but the last two; "a, b, c and 16 more": after each but the last
        const comma = !last && !(tail === null && index === shown.length - 2)
        return (
          <Fragment key={fact.key}>
            {index > 0 && last && tail === null ? " and " : " "}
            <CodeGlue after={comma ? "," : ""}>
              <button
                ref={index === places.length ? firstRest : undefined}
                type="button"
                // a declaration is code, as the story sets its names; a link between modules names its files
                className="rounded-sm font-mono outline-none hover:text-foreground hover:underline focus-visible:bd-focus disabled:no-underline"
                disabled={navigator === null}
                data-testid={testId}
                data-path={fact.place.path}
                title={fact.detail}
                onClick={() => navigator?.show(fact.place)}
              >
                <BreakableCode text={fact.label} />
              </button>
            </CodeGlue>
          </Fragment>
        )
      })}
      {tail !== null && shown.length > 0 && " and "}
      {tail !== null && shown.length === 0 && " "}
      {tail !== null &&
        (folded ? (
          <button
            type="button"
            className="rounded-sm text-foreground underline decoration-muted-foreground/60 underline-offset-2 outline-none hover:decoration-foreground focus-visible:bd-focus"
            data-testid="review-step-fact-more"
            onClick={() => setOpen(true)}
          >
            {tail}
          </button>
        ) : (
          tail
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
 * The warning of a step no test of the change reaches, accio having walked every way, and no scenario proves: "No test
 * of the change reaches paint", each name at its place in the code font
 */
export function UnprovenFact({ unproven, testId }: { unproven: NonNullable<StepFacts["unproven"]>; testId: string }) {
  return (
    <span className="text-xs text-warning" data-testid={testId}>
      <FactPlaces title="No test of the change reaches" list={unproven.names} testId="review-step-unreached" />
    </span>
  )
}
