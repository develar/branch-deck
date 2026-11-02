import type { CommitRole, ConcernName, ConcernPart, JoinedUnit, ReviewPlan, ReviewStep, StepCommit, StepSource, StoryRef } from "@/lib/bindings"
import { formatConcernEvidence, formatConcernName } from "@/presenters/review-concerns"

/*
 * The words of the story of a review (spec/branch/review-tour.spec.md, "In the Review Window"): Rust tells the steps
 * (a step per commit whose body tells it, of one commit a step per part of its message, else the concerns), what each
 * shows of the units in its order, the tests that name its code, and the brief; this words them for the tree, the
 * openings of the story and the bar above the diffs (the overview and the openings themselves: `review-story.ts`).
 */

/** The id Rust gives the step of the changes only commits without a body wrote */
const NO_STORY = "no-story"

/** Its heading: the changes only commits without a body wrote */
export const WITHOUT_A_STORY = "Changes without a story"

/**
 * A part of a chapter: a part of a unit it shows, with the units without a block of their own its blocks show
 * (`StepStop.joined`) and the lines of the told bodies that name it (`StepStop.marks`)
 */
export interface ChapterPart extends ConcernPart {
  readonly joined: readonly JoinedUnit[]
  readonly marks: readonly StoryRef[]
}

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
  /** What no evidence joined: the concern "Other changes" */
  readonly rest: boolean
  /**
   * Whether the story tells it as a step (its row a group, its opening, its place in the bar): not what no evidence
   * joined, and not the one concern that covers the whole range, which would only repeat the range
   */
  readonly told: boolean
  /** Its label is an identifier (the token, the declaration, the file of a concern no section of a body titles): code */
  readonly code: boolean
  /**
   * Its label is the first sentence of the paragraph its opening tells (`titleSource: "sentence"`): the opening shows
   * that paragraph whole, a step larger, and no title of its own
   */
  readonly titleInBody: boolean
  readonly parts: readonly ChapterPart[]
  readonly paths: readonly string[]
  readonly elsewhere: readonly string[]
}

/** The commit whose body tells a step: the one that starts it, else its first */
export function headOf(step: ReviewStep): StepCommit | undefined {
  return step.commits.find(commit => commit.role === "starts") ?? step.commits[0]
}

/** The chapters of the story of `plan`, from its steps in their order (`Chapter`) */
export function presentChapters(plan: Pick<ReviewPlan, "steps" | "concerns">): Chapter[] {
  const concerns = new Map(plan.concerns.map(concern => [concern.id, concern]))
  // of the steps of a message, a concern may be a step in part: its files are those of the step
  const message = plan.steps.source.kind === "message"
  return plan.steps.steps.map((step, index) => {
    const number = index + 1
    const parts = step.stops.map(stop => ({ unit: stop.unit, path: stop.path, blocks: stop.blocks, joined: stop.joined, marks: stop.marks }))
    const concern = step.concern === null ? undefined : concerns.get(step.concern)
    if (concern !== undefined) {
      // the author's name of the task, the label of the section of the body told in it, else the name of its evidence
      const label = step.title ?? formatConcernName(concern.name)
      const rest = concern.name.source === "rest"
      return {
        id: step.id,
        number,
        label,
        heading: label,
        reference: label,
        hover: formatConcernEvidence(concern.evidence),
        rest,
        told: !rest && plan.steps.told,
        code: step.title === null && isCodeName(concern.name),
        titleInBody: false,
        parts,
        paths: message ? step.paths : concern.paths,
        elsewhere: message ? step.elsewhere : concern.elsewhere,
      }
    }
    if (message) {
      // a part of the message of one commit: named as its author wrote it, by its label, its first sentence or its first
      // code name
      const label = step.title ?? ""
      return {
        id: step.id,
        number,
        label,
        heading: label,
        reference: label,
        hover: null,
        rest: false,
        told: plan.steps.told,
        code: step.titleSource === "name",
        titleInBody: step.titleSource === "sentence",
        parts,
        paths: step.paths,
        elsewhere: step.elsewhere,
      }
    }
    // Rust titles a step by the commit that starts it, as its author wrote it
    const label = step.id === NO_STORY ? WITHOUT_A_STORY : (step.title ?? headOf(step)?.title ?? "")
    return {
      id: step.id,
      number,
      label,
      heading: `${number} · ${label}`,
      reference: `step ${number}`,
      hover: null,
      rest: false,
      told: true,
      code: false,
      titleInBody: false,
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
  /** Its whole subject, in the hover of its row */
  readonly subject: string
  /** Its name on its row: the title Rust gives it (its subject without the issue key the brief names), else its subject */
  readonly title: string
  /** Its role in its step; null in Other commits */
  readonly role: CommitRole | null
  /** The commit of the range it is (`RangeReviewItem`); none when the range lists no such commit */
  readonly item: T | undefined
  /** Its place in the range: the index of its item, else after them all */
  readonly order: number
}

/**
 * Where each commit of the range is listed: the commit that starts each step of the commits, which the row of its step
 * is; the other commits of each step by its id; then those no step lists
 */
export interface CommitPlaces<T> {
  readonly heads: ReadonlyMap<string, PlacedCommit<T>>
  readonly steps: ReadonlyMap<string, readonly PlacedCommit<T>[]>
  readonly other: readonly PlacedCommit<T>[]
}

/** The same commit by two ids, one maybe short: the plan names a commit as its input did */
/**
 * The titles Rust gives the commits the steps of a plan list (`StepCommit.title`: the subject without its `(prefix)` and
 * the issue key of the brief), by id, added to `known`: a list of the commits (the commit column, the rail of a pull
 * request) names each commit as the tree and the openings do, also while it shows the plan of one commit alone. `known`
 * itself when the steps tell no new title
 */
export function withCommitTitles(known: ReadonlyMap<string, string>, steps: readonly Pick<ReviewStep, "commits">[]): ReadonlyMap<string, string> {
  let titles: Map<string, string> | null = null
  for (const commit of steps.flatMap(step => step.commits)) {
    if (known.get(commit.id) !== commit.title && titles?.get(commit.id) !== commit.title) {
      titles ??= new Map(known)
      titles.set(commit.id, commit.title)
    }
  }
  return titles ?? known
}

/** The title of `id` among `titles` (`withCommitTitles`), a short id or a full one alike */
export function commitTitleOf(titles: ReadonlyMap<string, string>, id: string): string | undefined {
  const exact = titles.get(id)
  if (exact !== undefined) {
    return exact
  }
  for (const [known, title] of titles) {
    if (sameCommit(known, id)) {
      return title
    }
  }
  return undefined
}

function sameCommit(a: string, b: string): boolean {
  return a.length > 0 && b.length > 0 && (a.startsWith(b) || b.startsWith(a))
}

/**
 * The commits of the range in the steps of the story (spec/branch/review-tour.spec.md, "In the Review Window"): a step
 * of the commits is the commit that starts it (`heads`) and lists the others, those that follow it up and those of no
 * story; each by the title Rust gives it, its subject without the issue key the brief names; a step of the concerns lists its commits, the
 * most lines first. A commit several steps list (the concerns) is listed once, in the step that lists it first (its most
 * lines there), the first of equal ones; a commit of the range no step lists goes into Other commits, in the order of
 * `items`. Of the steps of a message (`source`), no step is its commit: its one commit is listed in the first
 */
export function placeCommits<T extends { readonly commit: { readonly id: string; readonly subject: string } }>(
  steps: readonly Pick<ReviewStep, "id" | "commits" | "concern">[],
  items: readonly T[],
  source: StepSource["kind"] = "commits",
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
  const heads = new Map<string, PlacedCommit<T>>()
  const placed = new Map<string, PlacedCommit<T>[]>()
  const listed = new Set<string>()
  for (const step of steps) {
    const ofCommits = step.concern === null && source === "commits"
    const commits: PlacedCommit<T>[] = []
    for (const commit of step.commits) {
      const item = itemOf(commit)
      const id = item?.commit.id ?? commit.id
      if (home.get(id)?.step === step.id && !listed.has(id)) {
        listed.add(id)
        const it = { id, subject: item?.commit.subject ?? commit.title, title: commit.title, role: commit.role, item, order: orderOf(item) }
        if (ofCommits && commit.role === "starts" && !heads.has(step.id)) {
          heads.set(step.id, it)
        } else {
          commits.push(it)
        }
      }
    }
    if (commits.length > 0) {
      placed.set(step.id, commits)
    }
  }
  const other = items
    .filter(item => !listed.has(item.commit.id))
    .map(item => ({ id: item.commit.id, subject: item.commit.subject, title: item.commit.subject, role: null, item, order: orderOf(item) }))
  return { heads, steps: placed, other }
}

/** The role of a commit on its row in its step: "follows up", "no story"; nothing for the one that starts it */
export function formatCommitRole(role: CommitRole | null): string | null {
  return role === "followsUp" ? "follows up" : role === "noStory" ? "no story" : null
}

/** The chapters of `concerns` alone, each named by its evidence (the tests of the tree and the story without steps) */
export function concernChapters(concerns: Pick<ReviewPlan, "concerns">["concerns"]): Chapter[] {
  return concerns.map((concern, index) => {
    const label = formatConcernName(concern.name)
    const rest = concern.name.source === "rest"
    return {
      id: concern.id,
      number: index + 1,
      label,
      heading: label,
      reference: label,
      hover: formatConcernEvidence(concern.evidence),
      rest,
      told: !rest,
      code: isCodeName(concern.name),
      titleInBody: false,
      parts: concern.parts.map(part => ({ ...part, joined: NOT_JOINED, marks: NO_MARKS })),
      paths: concern.paths,
      elsewhere: concern.elsewhere,
    }
  })
}

const NOT_JOINED: readonly JoinedUnit[] = []
const NO_MARKS: readonly StoryRef[] = []

/** Whether the name of a concern is an identifier the code spells: a token, a declaration, a replacement, a file or a directory */
function isCodeName(name: ConcernName): boolean {
  return name.source !== "rest" && !(name.source === "directory" && name.directory === "")
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
