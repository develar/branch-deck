import type { CommitRole, ConcernPart, ContractKind, ReviewPlan, ReviewStep, StepCommit } from "@/lib/bindings"
import { formatConcernEvidence, formatConcernName } from "@/presenters/review-concerns"

/*
 * The words of the story of a review (spec/branch/review-tour.spec.md, "In the Review Window"): Rust tells the steps
 * (a step per commit whose body tells it, else the concerns), what each shows of the units in its order, the tests that
 * name its code, and the brief; this words them for the brief, the tree, the headings of the story and the bar above
 * the diffs.
 */

/** The id Rust gives the step of the changes only commits without a body wrote */
const NO_STORY = "no-story"

/** Its heading: the changes only commits without a body wrote */
export const WITHOUT_A_STORY = "Changes without a story"

/**
 * A chapter of the story: a step Rust told, the parts of the units it shows, its files and the main rows of them (a
 * file of several chapters has its row in one, a row "also in" the others). A step of the concerns is named by its
 * concern and keeps its main rows; `rest` is the concern no evidence joined, which the tree lists as Other changes
 */
export interface Chapter {
  readonly id: string
  /** Its place in the story, from 1 */
  readonly number: number
  /** Its name in the tree and the "also in" of its rows: the subject of the commit that starts it, else its concern */
  readonly label: string
  /** Its name at the head of its part of the story: "2 · The open asks the layout", a concern by its name */
  readonly heading: string
  /** Its name in the row of a block it shows that another section stands for: "step 2", a concern by its name */
  readonly reference: string
  /** Why its parts are together, in the hover of its row: the evidence of a concern */
  readonly hover: string | null
  readonly rest: boolean
  readonly parts: readonly ConcernPart[]
  readonly paths: readonly string[]
  readonly elsewhere: readonly string[]
}

/** The subject of a commit without the issue key the brief names already: "IJAI-1 the open asks" → "The open asks" */
export function formatStepTitle(subject: string, issue: string | null): string {
  const rest = issue !== null && subject.startsWith(issue) ? subject.slice(issue.length).replace(/^\s*[:,\-–—]?\s*/, "") : subject
  return rest.length === 0 ? subject : rest[0]!.toUpperCase() + rest.slice(1)
}

/** The commit whose body tells a step: the one that starts it, else its first */
export function headOf(step: ReviewStep): StepCommit | undefined {
  return step.commits.find(commit => commit.role === "starts") ?? step.commits[0]
}

/** The chapters of the story of `plan`, from its steps in their order (`Chapter`) */
export function presentChapters(plan: Pick<ReviewPlan, "steps" | "concerns" | "brief">): Chapter[] {
  const concerns = new Map(plan.concerns.map(concern => [concern.id, concern]))
  return plan.steps.steps.map((step, index) => {
    const number = index + 1
    const parts = step.stops.map(stop => ({ unit: stop.unit, path: stop.path, blocks: stop.blocks }))
    const concern = step.concern === null ? undefined : concerns.get(step.concern)
    if (concern !== undefined) {
      const label = formatConcernName(concern.name)
      return {
        id: step.id,
        number,
        label,
        heading: label,
        reference: label,
        hover: formatConcernEvidence(concern.evidence),
        rest: concern.name.source === "rest",
        parts,
        paths: concern.paths,
        elsewhere: concern.elsewhere,
      }
    }
    const label = step.id === NO_STORY ? WITHOUT_A_STORY : formatStepTitle(headOf(step)?.subject ?? "", plan.brief.issue)
    return {
      id: step.id,
      number,
      label,
      heading: `${number} · ${label}`,
      reference: `step ${number}`,
      hover: null,
      rest: false,
      parts,
      paths: step.paths,
      elsewhere: step.elsewhere,
    }
  })
}

/**
 * The marks of an owner chip on the steps (spec/branch/branch-review.spec.md, "Code owners"): for each chapter by its
 * id, how many of the files it shows (its own rows and those "also in" it) the chip marks; 0 mutes the step
 */
export function presentOwnerMarks(chapters: readonly Chapter[], paths: ReadonlySet<string>): Map<string, number> {
  return new Map(chapters.map(chapter => [chapter.id, new Set(chapter.paths.filter(path => paths.has(path))).size]))
}

/** A commit of the range as a row of the tree: in the step it tells or follows up, else in Other commits */
export interface PlacedCommit<T> {
  /** The id of the commit: its item's, else the step's */
  readonly id: string
  readonly subject: string
  /** Its role in its step; null in Other commits */
  readonly role: CommitRole | null
  /** The commit of the range it is (`RangeReviewItem`); none when the range lists no such commit */
  readonly item: T | undefined
  /** Its place in the range: the index of its item, else after them all */
  readonly order: number
}

/** Where each commit of the range is listed: the commits of each step by its id, then those no step lists */
export interface CommitPlaces<T> {
  readonly steps: ReadonlyMap<string, readonly PlacedCommit<T>[]>
  readonly other: readonly PlacedCommit<T>[]
}

/** The same commit by two ids, one maybe short: the plan names a commit as its input did */
function sameCommit(a: string, b: string): boolean {
  return a.length > 0 && b.length > 0 && (a.startsWith(b) || b.startsWith(a))
}

/**
 * The commits of the range in the steps of the story (spec/branch/review-tour.spec.md, "In the Review Window"): each step
 * lists its commits in its order (the one that starts it, those that follow it up, those of no story; of a concern the
 * most lines first). A commit several steps list (the concerns) is listed once, in the step that lists it first (its
 * most lines there), the first of equal ones; a commit of the range no step lists goes into Other commits, in the order
 * of `items`
 */
export function placeCommits<T extends { readonly commit: { readonly id: string; readonly subject: string } }>(
  steps: readonly Pick<ReviewStep, "id" | "commits">[],
  items: readonly T[],
): CommitPlaces<T> {
  const itemOf = (commit: StepCommit) => items.find(item => sameCommit(item.commit.id, commit.id))
  const orderOf = (item: T | undefined) => (item === undefined ? items.length : items.indexOf(item))
  // the step each commit is listed in: the lowest place in a step's list, the first step of equal ones
  const home = new Map<string, { step: string; at: number }>()
  for (const step of steps) {
    step.commits.forEach((commit, at) => {
      const id = itemOf(commit)?.commit.id ?? commit.id
      const known = home.get(id)
      if (known === undefined || at < known.at) {
        home.set(id, { step: step.id, at })
      }
    })
  }
  const placed = new Map<string, PlacedCommit<T>[]>()
  const listed = new Set<string>()
  for (const step of steps) {
    const commits: PlacedCommit<T>[] = []
    for (const commit of step.commits) {
      const item = itemOf(commit)
      const id = item?.commit.id ?? commit.id
      if (home.get(id)?.step === step.id && !listed.has(id)) {
        listed.add(id)
        commits.push({ id, subject: item?.commit.subject ?? commit.subject, role: commit.role, item, order: orderOf(item) })
      }
    }
    if (commits.length > 0) {
      placed.set(step.id, commits)
    }
  }
  const other = items.filter(item => !listed.has(item.commit.id)).map(item => ({ id: item.commit.id, subject: item.commit.subject, role: null, item, order: orderOf(item) }))
  return { steps: placed, other }
}

/** The role of a commit on its row in its step: "follows up", "no story"; nothing for the one that starts it */
export function formatCommitRole(role: CommitRole | null): string | null {
  return role === "followsUp" ? "follows up" : role === "noStory" ? "no story" : null
}

/** The chapters of `concerns` alone, each named by its evidence (the tests of the tree and the story without steps) */
export function concernChapters(concerns: Pick<ReviewPlan, "concerns">["concerns"]): Chapter[] {
  return concerns.map((concern, index) => {
    const label = formatConcernName(concern.name)
    return {
      id: concern.id,
      number: index + 1,
      label,
      heading: label,
      reference: label,
      hover: formatConcernEvidence(concern.evidence),
      rest: concern.name.source === "rest",
      parts: concern.parts,
      paths: concern.paths,
      elsewhere: concern.elsewhere,
    }
  })
}

/** A step of the brief, worded */
export interface BriefStep {
  readonly id: string
  readonly number: number
  readonly title: string
  /** The first paragraph of the body of the commit that starts it, the why; null without a body */
  readonly why: string | null
  /** The rest of its body, paragraph by paragraph */
  readonly more: readonly string[]
  /** The other commits of the step: "Follows up: …", "Without a story: …" */
  readonly rounds: readonly string[]
  /** Its code that tests the change adds or changes reach: "Tests of this change reach open, allowed" */
  readonly proven: string | null
  /** Its code that tests the change did not touch reach: "2 tests it did not touch reach focus", "1 test it did not touch reaches focus" */
  readonly covered: string | null
  /**
   * accio answered, and no test reaches any of its code and no scenario proves it: "No test reaches paint", "No test
   * reaches paint within 3 calls" when the walk stopped with callers left
   */
  readonly unproven: string | null
  /** Its code no test reaches, beside code a test reaches or a scenario: "No test reaches isEmpty" */
  readonly unreached: string | null
  /** "Proven by the scenario restart.json" */
  readonly scenarios: string | null
  /** Nothing of it lands: a later commit took it out */
  readonly landsNothing: boolean
}

/** An entry of "What changes" */
export interface BriefContract {
  readonly key: string
  readonly kind: string
  readonly path: string
  readonly heading: string | null
  readonly paragraphs: readonly string[]
  readonly removed: readonly string[]
  readonly provenBy: string | null
}

/** The brief, worded: the issue, what changes, the steps, and why the steps are the concerns when they are */
export interface BriefView {
  readonly issue: string | null
  readonly contract: readonly BriefContract[]
  readonly steps: readonly BriefStep[]
  /** Why the tasks of the range are its steps: "One commit: its tasks are the steps" */
  readonly source: string | null
}

const CONTRACT_WORDS: Readonly<Record<ContractKind, string>> = {
  spec: "Spec rule",
  decision: "Decision",
  guide: "Agent guide",
  doc: "Doc",
}

/** The paragraphs of a commit body: a blank line ends one, its lines joined */
export function bodyParagraphs(body: string): string[] {
  return body
    .split(/\n\s*\n/)
    .map(paragraph =>
      paragraph
        .split("\n")
        .map(line => line.trim())
        .join(" ")
        .trim(),
    )
    .filter(paragraph => paragraph.length > 0)
}

/** The names of a list, the first `shown` of them and how many more: "run, create +2" */
function names(list: readonly string[], shown = 4): string {
  const rest = list.length - shown
  return rest > 0 ? `${list.slice(0, shown).join(", ")} +${rest}` : list.join(", ")
}

/** The name of a file of a path: its last part */
function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1)
}

/** The brief of `plan`, in the order Rust gave it (`BriefView`); no step when the only one is what no evidence joined */
export function presentBrief(plan: Pick<ReviewPlan, "steps" | "concerns" | "brief">): BriefView {
  const chapters = presentChapters(plan)
  const told = chapters.some(chapter => !chapter.rest)
  const steps = (told ? plan.steps.steps : []).map((step, index): BriefStep => {
    const head = headOf(step)
    const paragraphs = head === undefined ? [] : bodyParagraphs(head.body)
    const rounds = step.commits
      .filter(commit => commit !== head)
      .map(commit => `${commit.role === "noStory" ? "Without a story" : "Follows up"}: ${formatStepTitle(commit.subject, plan.brief.issue)}`)
    const { answered, reached, unreached, scenarios } = step.tests
    const ofChange = reached.filter(declaration => declaration.tests.some(test => test.unit !== null))
    const untouched = new Set(reached.flatMap(declaration => declaration.tests.filter(test => test.unit === null).map(test => `${test.path}:${test.line}`)))
    const coveredNames = reached.filter(declaration => declaration.tests.some(test => test.unit === null)).map(it => it.name)
    const within = Math.max(0, ...unreached.map(declaration => declaration.within ?? 0))
    const none = unreached.length === 0 ? null : `No test reaches ${names(unreached.map(it => it.name))}`
    return {
      id: step.id,
      number: index + 1,
      title: chapters[index]!.label,
      why: paragraphs[0] ?? null,
      more: paragraphs.slice(1),
      rounds,
      proven: ofChange.length === 0 ? null : `Tests of this change reach ${names(ofChange.map(it => it.name))}`,
      covered:
        untouched.size === 0 ? null : `${untouched.size === 1 ? "1 test it did not touch reaches" : `${untouched.size} tests it did not touch reach`} ${names(coveredNames)}`,
      unproven: answered && none !== null && reached.length === 0 && scenarios.length === 0 ? (within > 0 ? `${none} within ${within} calls` : none) : null,
      unreached: none !== null && (reached.length > 0 || scenarios.length > 0) ? none : null,
      scenarios: scenarios.length === 0 ? null : `Proven by ${names(scenarios.map(fileName))}`,
      landsNothing: step.stops.length === 0,
    }
  })
  const source =
    !told || plan.steps.source.kind === "commits"
      ? null
      : plan.steps.source.reason === "oneCommit"
        ? "One commit: the steps are its tasks"
        : "The commits tell no story: the steps are the tasks of the range"
  return {
    issue: plan.brief.issue,
    contract: plan.brief.contract.map((entry, index) => ({
      key: `${entry.unit}:${entry.block ?? "file"}:${index}`,
      kind: CONTRACT_WORDS[entry.kind],
      path: entry.path,
      heading: entry.heading,
      paragraphs: entry.newSpec !== null ? [entry.newSpec.description ?? `A new spec of ${entry.newSpec.rules} rules`] : entry.deleted ? ["Deleted"] : entry.added,
      removed: entry.removed,
      provenBy: entry.provenBy.length === 0 ? null : `Proven by ${names(entry.provenBy.map(fileName))}`,
    })),
    steps,
    source,
  }
}

/** The words of a step in the diffs, in the hover of the map of the files: "Step 2 · The open keeps a stored layout" */
export function formatStepDivider(chapter: Pick<Chapter, "number" | "label">): string {
  return `${formatStepNumber(chapter.number)} · ${chapter.label}`
}

/** The number of a step, which the divider above its first section mutes before its title: "Step 2" */
export function formatStepNumber(number: number): string {
  return `Step ${number}`
}

/** The bar above the diffs: the step of the section at the top, "Step 2 / 4" and its title */
export function formatStepPosition(number: number, count: number): string {
  return `Step ${number} / ${count}`
}
