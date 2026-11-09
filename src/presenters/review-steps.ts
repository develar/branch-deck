import type {
  CommitRole,
  ConcernName,
  ConcernPart,
  FileFacts,
  GroupKind,
  JoinedUnit,
  MechanicalKind,
  ModuleShape,
  ReviewPlan,
  ReviewStep,
  StepCommit,
  StepSource,
  StoryRef,
} from "@/lib/bindings"
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
  /**
   * Of the first part of a group of a large step (`ReviewStep.groups`), and of the first test of a group: the words of
   * the divider above its card ("view.gui.media · New module · 59 files", "view.gui.media · Tests · 18 files"); null
   * for the other parts, for a step read as one list and for the mechanical group, whose kinds head their sections
   */
  readonly groupHeading: string | null
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
  /** The groups of a large step Rust cut (`ReviewStep.groups`), none for a step read as one list and for a concern */
  readonly groups: readonly ChapterGroup[]
}

/**
 * A group of a large step as the tree reads it (spec/branch/review-tour.spec.md, "Groups of a Step"): a section of its
 * step, which lists the files of its stops, each once, at the first of them, a file Rust classed mechanical in the
 * mechanical group
 */
export interface ChapterGroup {
  readonly kind: GroupKind
  /** Its name on its row: "Contract", the label of its module, its folder; the mechanical group "Wiring" or "Mechanical" */
  readonly name: string
  /** Its name is the label of a module: code */
  readonly code: boolean
  /** Its hover: the root of its module, of a folder the folder there, else what it holds */
  readonly hover: string
  /** What the commits do to its module, when they add or delete it */
  readonly state: "added" | "deleted" | null
  /** The small modules whose files it holds: "also shared/code" */
  readonly also: string | null
  /** The root of its module; null for the contract and the mechanical group */
  readonly root: string | null
  /** Its files (its rows and its rows "also in" another step) in the order of its stops; those from `tests` on are its tests */
  readonly paths: readonly string[]
  readonly tests: number
  /**
   * The directory after the name of each of its files: of a file of its module the one Rust gives it below what the
   * files of the module share (`FileFacts.folder`: `ui/embeds` of its code and of its tests, "" in its own), of a file
   * of a small module the label of that module; null for the whole directory (the contract, the mechanical group)
   */
  readonly directories: readonly (string | null)[]
}

const NO_GROUPS: readonly ChapterGroup[] = []

/** The kinds of the mechanical files that wire modules in: the build model and the lists of modules */
const WIRING: ReadonlySet<MechanicalKind> = new Set(["buildModel", "moduleList"])

/**
 * The groups of `step` (`ChapterGroup`): a file in the group of its first stop, a mechanical file (`mechanical`) in the
 * mechanical group; a group that lists no file left out. The mechanical group is "Wiring" while its files are the build
 * model and the lists of modules, else "Mechanical"
 */
function chapterGroups(
  step: ReviewStep,
  modules: readonly ModuleShape[],
  mechanical: ReadonlyMap<string, MechanicalKind>,
  moduleOf: ReadonlyMap<string, number>,
  folderOf: ReadonlyMap<string, string>,
): readonly ChapterGroup[] {
  if (step.groups.length === 0) {
    return NO_GROUPS
  }
  const folds = step.groups.some(group => group.kind === "mechanical")
  const placed = new Set<string>()
  const groups: ChapterGroup[] = []
  for (const group of step.groups) {
    const paths: string[] = []
    let tests = -1
    for (let at = group.start; at < group.end; at++) {
      if (at === group.tests) {
        tests = paths.length
      }
      const path = step.stops[at]?.path
      if (path !== undefined && !placed.has(path) && (group.kind === "mechanical" || !folds || !mechanical.has(path))) {
        placed.add(path)
        paths.push(path)
      }
    }
    if (paths.length > 0) {
      const words = groupWords(group.kind, group.module === null ? undefined : modules[group.module], group.folder, paths, mechanical)
      const directories =
        words.root === null
          ? paths.map(() => null)
          : paths.map(path => (moduleOf.get(path) === group.module ? (folderOf.get(path) ?? "") : (modules[moduleOf.get(path) ?? -1]?.label ?? null)))
      groups.push({ ...words, also: alsoOf(group.also, modules), paths, tests: tests < 0 ? paths.length : tests, directories })
    }
  }
  return groups
}

function groupWords(
  kind: GroupKind,
  module: ModuleShape | undefined,
  folder: string | null,
  paths: readonly string[],
  mechanical: ReadonlyMap<string, MechanicalKind>,
): Pick<ChapterGroup, "kind" | "name" | "code" | "hover" | "state" | "root"> {
  if (kind === "contract") {
    return { kind, name: "Contract", code: false, hover: "The specs and the docs the step changes", state: null, root: null }
  }
  if (kind === "mechanical") {
    const wiring = paths.every(path => {
      const of = mechanical.get(path)
      return of !== undefined && WIRING.has(of)
    })
    return wiring
      ? { kind, name: "Wiring", code: false, hover: "The build files and the lists of modules of the step", state: null, root: null }
      : {
          kind,
          name: "Mechanical",
          code: false,
          hover: "The mechanical changes of the step: the build model, imports alone, whitespace, renames, generated files",
          state: null,
          root: null,
        }
  }
  if (module === undefined) {
    return { kind, name: "Other files", code: false, hover: "The files of no module", state: null, root: null }
  }
  const state = module.state === "changed" ? null : module.state
  const root = module.root === "" ? "The root of the repository" : module.root
  if (kind === "folder" && folder !== null && folder !== "") {
    return { kind, name: folder, code: false, hover: `${root} · ${folder}`, state, root: module.root }
  }
  return { kind, name: module.label, code: true, hover: root, state, root: module.root }
}

/** "also shared/code": the labels of the small modules a group holds */
function alsoOf(also: readonly number[], modules: readonly ModuleShape[]): string | null {
  const labels = also.flatMap(module => (modules[module] === undefined ? [] : [modules[module].label]))
  return labels.length === 0 ? null : `also ${labels.join(", ")}`
}

/** The badge of the module of a group: "New module", "Deleted module" */
export function formatModuleState(state: "added" | "deleted"): string {
  return state === "added" ? "New module" : "Deleted module"
}

/** The files of a group or a step the reader saw: "3 / 11 seen" */
export function formatSeen(seen: number, total: number): string {
  return `${seen} / ${total} seen`
}

/** The heading of the tests of a group, at its end */
export const GROUP_TESTS = "Tests"

/** The commit whose body tells a step: the one that starts it, else its first */
export function headOf(step: ReviewStep): StepCommit | undefined {
  return step.commits.find(commit => commit.role === "starts") ?? step.commits[0]
}

/** The chapters of the story of `plan`, from its steps in their order (`Chapter`) */
export function presentChapters(plan: Pick<ReviewPlan, "steps" | "concerns"> & { readonly modules?: readonly ModuleShape[]; readonly files?: readonly FileFacts[] }): Chapter[] {
  const concerns = new Map(plan.concerns.map(concern => [concern.id, concern]))
  const modules = plan.modules ?? []
  const mechanical = new Map((plan.files ?? []).flatMap(file => (file.mechanical === null ? [] : [[file.path, file.mechanical] as const])))
  const moduleOf = new Map((plan.files ?? []).flatMap(file => (file.module === null ? [] : [[file.path, file.module] as const])))
  const folderOf = new Map((plan.files ?? []).map(file => [file.path, file.folder] as const))
  // of the steps of a message, a concern may be a step in part: its files are those of the step
  const message = plan.steps.source.kind === "message"
  return plan.steps.steps.map((step, index) => {
    const number = index + 1
    const headings = groupHeadings(step, plan.modules ?? [])
    const parts = step.stops.map((stop, at) => ({
      unit: stop.unit,
      path: stop.path,
      blocks: stop.blocks,
      joined: stop.joined,
      marks: stop.marks,
      groupHeading: headings.get(at) ?? null,
    }))
    const concern = step.concern === null ? undefined : concerns.get(step.concern)
    if (concern !== undefined) {
      // the author's name of the task, the label of the section of the body told in it, else the name of its evidence
      const label = step.title ?? formatConcernName(concern.name)
      const rest = concern.name.source === "rest"
      const told = !rest && plan.steps.told
      return {
        id: step.id,
        number,
        label,
        // a step the overview tells by its number is that step everywhere: "2 · …" in the tree, "in step 2" after a row
        heading: told ? `${number} · ${label}` : label,
        reference: told ? `step ${number}` : label,
        hover: formatConcernEvidence(concern.evidence),
        rest,
        told,
        code: step.title === null && isCodeName(concern.name),
        titleInBody: false,
        parts,
        paths: message ? step.paths : concern.paths,
        elsewhere: message ? step.elsewhere : concern.elsewhere,
        groups: NO_GROUPS,
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
        heading: plan.steps.told ? `${number} · ${label}` : label,
        reference: plan.steps.told ? `step ${number}` : label,
        hover: null,
        rest: false,
        told: plan.steps.told,
        code: step.titleSource === "name" && !label.includes(" "),
        titleInBody: step.titleSource === "sentence",
        parts,
        paths: step.paths,
        elsewhere: step.elsewhere,
        groups: chapterGroups(step, modules, mechanical, moduleOf, folderOf),
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
      groups: chapterGroups(step, modules, mechanical, moduleOf, folderOf),
    }
  })
}

/**
 * The words of the divider of each group of a large step (spec/branch/review-tour.spec.md, "Groups of a Step"), by the
 * place of its first stop, and of the tests of a group by the place of the first: the contract, a module by its label
 * ("New module" when the change adds it, the small modules it holds), a folder; the mechanical group none, the kinds of
 * its sections head them
 */
export function groupHeadings(step: ReviewStep, modules: readonly ModuleShape[]): Map<number, string> {
  const headings = new Map<number, string>()
  const filesOf = (start: number, end: number) => {
    const count = new Set(step.stops.slice(start, end).map(stop => stop.path)).size
    return count === 1 ? "1 file" : `${count} files`
  }
  for (const group of step.groups) {
    const module = group.module === null ? undefined : modules[group.module]
    const name =
      group.kind === "contract"
        ? "Contract"
        : group.kind === "folder"
          ? group.folder === null || group.folder === ""
            ? (module?.label ?? "")
            : group.folder
          : (module?.label ?? "Other files")
    if (group.kind === "mechanical") {
      continue
    }
    if (group.tests > group.start) {
      const state = module?.state === "added" ? " · New module" : module?.state === "deleted" ? " · Deleted module" : ""
      const also = group.also.flatMap(place => modules[place]?.label ?? [])
      headings.set(group.start, `${name}${state} · ${filesOf(group.start, group.tests)}${also.length === 0 ? "" : ` · also ${also.join(", ")}`}`)
    }
    if (group.end > group.tests) {
      headings.set(group.tests, `${name} · Tests · ${filesOf(group.tests, group.end)}`)
    }
  }
  return headings
}

/**
 * The marks of an owner chip on the steps (spec/branch/branch-review.spec.md, "Code owners"): for each chapter by its
 * id, the files it shows (its own rows and those "also in" it) and the ones the chip marks
 */
export function presentOwnerMarks(chapters: readonly Chapter[], paths: ReadonlySet<string>): Map<string, { shown: number; total: number }> {
  return new Map(chapters.map(chapter => [chapter.id, { shown: new Set(chapter.paths.filter(path => paths.has(path))).size, total: new Set(chapter.paths).size }]))
}

/**
 * The files of `paths` each group of a step lists, of every step with groups (`ChapterGroup`): the files of an owner chip
 * (spec/branch/branch-review.spec.md, "Code owners"), the files the reader saw ("In the Review Window" of
 * spec/branch/review-tour.spec.md); each by its step and its place among the groups of it
 */
export function presentGroupCounts(chapters: readonly Chapter[], paths: ReadonlySet<string>): { chapter: string; group: number; shown: number; total: number }[] {
  return chapters.flatMap(chapter =>
    chapter.groups.map((group, index) => ({ chapter: chapter.id, group: index, shown: group.paths.filter(path => paths.has(path)).length, total: group.paths.length })),
  )
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
      parts: concern.parts.map(part => ({ ...part, joined: NOT_JOINED, marks: NO_MARKS, groupHeading: null })),
      paths: concern.paths,
      elsewhere: concern.elsewhere,
      groups: NO_GROUPS,
    }
  })
}

const NOT_JOINED: readonly JoinedUnit[] = []
const NO_MARKS: readonly StoryRef[] = []

/** Whether the name of a concern is an identifier the code spells: a token, a declaration, a replacement, a file or a directory */
function isCodeName(name: ConcernName): boolean {
  return name.source !== "rest" && !(name.source === "directory" && name.directory === "") && !(name.source === "declaration" && name.name.includes(" "))
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
