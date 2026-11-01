import type { CommitRole, ConcernPart, ReviewPlan, ReviewStep, StepCommit } from "@/lib/bindings"
import { formatConcernEvidence, formatConcernName } from "@/presenters/review-concerns"

/*
 * The words of the story of a review (spec/branch/review-tour.spec.md, "In the Review Window"): Rust tells the steps
 * (a step per commit whose body tells it, else the concerns), what each shows of the units in its order, the tests that
 * name its code, and the brief; this words them for the tree, the openings of the story and the bar above the diffs
 * (the overview and the openings themselves: `review-story.ts`).
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

/** The bar above the diffs while the overview is at the top: "4 steps" */
export function formatStepCount(count: number): string {
  return count === 1 ? "1 step" : `${count} steps`
}
