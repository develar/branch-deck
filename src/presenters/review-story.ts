import type {
  ContractEntry,
  ApiChange,
  CodeSpan,
  CommitBodyBlock,
  CommitRole,
  ConcernsReason,
  ContractKind,
  Mention,
  MentionTarget,
  MessageChange,
  ProseChange,
  ProseForm,
  ReviewPlan,
  ReviewStep,
  ReviewUnit,
  StepSource,
  StepTest,
  StoryCommit,
  StoryPart,
  TextRange,
} from "@/lib/bindings"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import { cutList, joinNames } from "@/lib/text"
import type { BodyPart, BodyParts, PlacedBlock } from "@/presenters/commit-body"
import { codeOf, presentBodyParts } from "@/presenters/commit-body"
import type { EvidencePart } from "@/presenters/review-concerns"
import { presentConcernEvidence } from "@/presenters/review-concerns"
import type { MapRegistration, MapTexts, MapView } from "@/presenters/review-map"
import { moduleRegistrations, presentMap, textsOf } from "@/presenters/review-map"
import type { Breakage } from "@/presenters/review-plan"
import { firstUse, formatPairState, formatRewrittenChanges, formatSinceReview, presentBreakage } from "@/presenters/review-plan"
import { alsoOf, formatModuleState, formatStepNumber, groupOpeningKey, headOf, opensGroup, presentChapters } from "@/presenters/review-steps"
import type { NarrativeSection } from "@/presenters/review-units"

/*
 * The words of the review as one document (spec/branch/review-tour.spec.md, "In the Review Window"): the overview at
 * its top (`presentPrologue`), then the opening of each step above the card of its first section (`presentOpenings`).
 * Rust tells the steps, the story of each commit by the grammar of its body, its code names, where the fold of a body
 * is, where each part is told and the sections each line of a story names; this only words and places them.
 */

/**
 * A code name of a body (spec/branch/review-tour.spec.md, "Anchors"): its place in the diffs, which a click shows, and
 * the words of its hover ("New · Manager.kt:6 · in step 2"); a name that resolves nothing has neither and stays code in
 * the text
 */
export interface StoryMention extends Mention {
  readonly place: ReviewPlace | null
  /**
   * The card of its hover in one line, the description of the link for a screen reader, whose name is the name it shows:
   * "New · Manager.kt:6", "· in step 2" when another step tells it
   */
  readonly description: string | null
  /** The card of its hover: what the target is, where, and its step when another step tells it; null without a target */
  readonly card: MentionCard | null
}

/**
 * The card of the hover of a code name (spec/branch/review-tour.spec.md, "Anchors"): never the name again, which the
 * link shows, but what it names: the status of a declaration ("New", "Removed", "Body changed"), its file and line,
 * and "in step 5" when the step that shows it is not the one the reader is in
 */
export interface MentionCard {
  readonly status: string | null
  readonly tone: "success" | "destructive" | "secondary"
  readonly location: string
  readonly step: string | null
}

/** The words and the place of the target of a mention, by the units and the steps of the plan */
export type MentionWords = (target: MentionTarget, text: string) => { readonly place: ReviewPlace; readonly description: string; readonly card: MentionCard }

/** The body of a commit (or the blocks of it told at one place) as the document shows it: its parts, its code names */
export interface StoryView extends BodyParts {
  readonly commit: string
  /** The code names of the whole body: a line finds its own by its block and its item */
  readonly mentions: readonly StoryMention[]
  /** The subject its reviewed version had, when a reword since the review changed it (`MessageChange.subjectBefore`) */
  readonly subjectBefore?: string
}

/**
 * The story of `commit` as the document shows it: of its parts `told` keeps (all by default), each as Rust cut, placed
 * and folded it (`StoryCommit.parts`, `presentBodyParts`), its code names worded
 */
export function presentStory(commit: StoryCommit, told: (part: StoryPart) => boolean = () => true, words: MentionWords = plainWords, titled = false): StoryView {
  const subjectBefore = commit.change?.subjectBefore ?? null
  const parts = commit.parts.filter(told)
  return {
    commit: commit.id,
    mentions: wordMentions(commit, words),
    ...presentBodyParts(commit.id, commit.body, titled ? titleAsLead(parts) : parts, true, commit.change),
    ...(subjectBefore === null ? {} : { subjectBefore }),
  }
}

/**
 * The parts a step tells whose first sentence titles it (`Chapter.titleInBody`): the paragraph of that sentence is
 * their lead, a step larger, so the opening reads it as its title; the blocks written under it follow as its run
 */
function titleAsLead(parts: readonly StoryPart[]): StoryPart[] {
  const [first, ...others] = parts
  if (first?.kind !== "run") {
    return [...parts]
  }
  const lead: StoryPart = { ...first, kind: "lead", end: first.start + 1 }
  return first.end > first.start + 1 ? [lead, { ...first, start: first.start + 1, removals: [] }, ...others] : [lead, ...others]
}

/** The mentions of code of the body of `commit`, each worded by `words` (`StoryMention`) */
function wordMentions(commit: StoryCommit, words: MentionWords): StoryMention[] {
  return commit.mentions.map(mention => ({
    ...mention,
    ...(mention.target === null ? { place: null, description: null, card: null } : words(mention.target, mention.text)),
  }))
}

/** The words of a target without the units: its file and line */
function plainWords(target: MentionTarget): ReturnType<MentionWords> {
  if (target.kind === "file") {
    return withDescription(fileTop(target.path), { status: null, tone: "secondary", location: target.path, step: null })
  }
  const location = target.line === null ? fileName(target.path) : `${fileName(target.path)}:${target.line}`
  return withDescription({ path: target.path, side: target.side, line: target.line, name: null }, { status: null, tone: "secondary", location, step: null })
}

/** The place and the card of a mention with its description, the card in one line: the screen reader hears what the hover shows */
function withDescription(place: ReviewPlace, card: MentionCard): ReturnType<MentionWords> {
  return { place, card, description: [card.status, card.location, card.step].filter(part => part !== null).join(" · ") }
}

/** What a unit is to the change, the badge of the card of a name: "New", "Removed", "Body changed", "Form changed" */
function unitStatus(unit: ReviewUnit): Pick<MentionCard, "status" | "tone"> {
  switch (unit.kind.kind) {
    case "added":
      return { status: "New", tone: "success" }
    case "newFile":
      return { status: "New file", tone: "success" }
    case "removed":
      return { status: "Removed", tone: "destructive" }
    case "deletedFile":
    case "deletedFiles":
      return { status: "Deleted file", tone: "destructive" }
    case "behaviour":
      return { status: "Body changed", tone: "secondary" }
    case "shape":
      return { status: "Form changed", tone: "secondary" }
    default:
      return { status: null, tone: "secondary" }
  }
}

/**
 * The words of the targets of the mentions of `plan`, the place a click shows (of a declaration the name Rust resolved
 * selected once shown) and the card of the hover: the status of a declaration, its file and line, a file by its path,
 * and the step only when it is not `current` (the place of the step whose opening tells the line, null for the
 * overview); the description says the card in one line ("New file · LayoutStore.kt:4 · in step 5")
 */
export function mentionWords(plan: Pick<ReviewPlan, "units">, current: number | null = null): MentionWords {
  const units = new Map(plan.units.map(unit => [unit.id, unit]))
  const elsewhere = (index: number | null) => (index === null || index === current ? null : `in step ${index + 1}`)
  return target => {
    if (target.kind === "file") {
      return withDescription(fileTop(target.path), { status: null, tone: "secondary", location: target.path, step: elsewhere(target.step) })
    }
    const unit = units.get(target.unit)
    const { place, card } = plainWords(target)
    return withDescription({ ...place, name: target.name }, { ...card, ...(unit === undefined ? {} : unitStatus(unit)), step: elsewhere(target.step) })
  }
}

/** The story Rust tells by its parts, each at its place (the concerns of one commit): its parts in the overview and in steps */
function storyByParts(plan: Pick<ReviewPlan, "steps">): StoryCommit | null {
  return plan.steps.commits.find(commit => commit.parts.some(part => part.told.kind !== "commit")) ?? null
}

/** Whether Rust tells `part` in the opening of the step at `index` */
function toldInStep(part: StoryPart, index: number): boolean {
  return part.told.kind === "step" && part.told.step === index
}

/** Why the tasks of the range are its steps, under the label of the steps */
const SOURCE_WORDS: Readonly<Record<ConcernsReason, string>> = {
  oneCommit: "One commit: the steps are its tasks",
  noStory: "The commits tell no story: the steps are the tasks of the range",
}

/** Why the steps are what they are, under the label of the steps; nothing for the steps of the commits */
function formatStepSource(source: StepSource): string | null {
  switch (source.kind) {
    case "commits":
      return null
    case "message":
      return "One commit: the steps are the parts of its message"
    case "concerns":
      return SOURCE_WORDS[source.reason]
  }
}

/**
 * Whether `step` tells parts of the one story of the range (`storyByParts`): a concern of one commit, a part of its
 * message; else it tells the body of the commit that starts it
 */
function tellsParts(plan: Pick<ReviewPlan, "steps">, step: ReviewStep): boolean {
  return step.concern !== null || plan.steps.source.kind === "message"
}

/**
 * A name of a fact of a step that shows its place: a file of tests of the change, a declaration, a new link between
 * modules at its first use
 */
export interface FactPlace {
  readonly key: string
  /** "Cache", "Reader.kt → Cache.kt", "ManagerTest.kt" */
  readonly label: string
  readonly place: ReviewPlace
  /** Its hover: of a file of tests, the tests it holds that reach the step, a line each, and what each reaches */
  readonly detail?: string
}

/**
 * The names of a fact of a step, each at its place, and how many more it has than it names: the contents cut a long
 * list as every list is ("Cache, Reader, Store and 16 more", `cutList`), the opening names them all
 */
export interface FactList {
  readonly places: readonly FactPlace[]
  /** Its names left out of `places`; 0 for a list named whole */
  readonly more: number
  /**
   * In the opening of a step, the names left out of `places`, which "and N more" shows in place: the opening keeps one
   * screen and still names every one on a click; absent in the contents, whose cut is final (the opening tells the rest)
   */
  readonly rest?: readonly FactPlace[]
}

/**
 * A fact of the tests of a step, a line of its own in its opening: its words and the names it lists, each at its place
 * ("Tested by ManagerTest.kt and LayoutTest.kt", "No test of the change reaches show")
 */
export interface TestFact {
  /**
   * "change": the files of the tests of the change that reach its code; "none": its code no test of the change
   * reaches, accio having walked every way
   */
  readonly key: "change" | "none"
  readonly words: string
  readonly names: FactList
}

/** "Tested by ManagerTest.kt", "Proven by restart.json": the facts of a step, and its warning (`presentStepFacts`) */
export interface StepFacts {
  /**
   * Its facts in one line: in the contents how many of its declarations the tests reach, the scenarios that prove it and
   * that nothing of it lands; in the opening the scenarios and that nothing lands, its tests in `tests`; null for none
   */
  readonly line: string | null
  /**
   * The tests that reach its code and its code none reaches, each name at its place: in the opening a line each, every
   * name; in the contents the files of the tests of the change and its code none reaches beside a scenario, a long list
   * cut
   */
  readonly tests: readonly TestFact[]
  /** Of a step of tests: the other steps whose code its tests reach ("Proves step 2"), each showing that step */
  readonly proves: readonly { readonly key: string; readonly step: string; readonly stepId: string }[]
  /** The public declarations it adds to a module that was there ("New API:"), each at its declaration */
  readonly newApi: FactList
  /**
   * The public declarations it adds in a module the change adds, counted, not named: the whole surface of a new module
   * is new ("New module view.gui.media: 13 declarations"), the names in the hover of its label, which shows the first
   */
  readonly newModules: readonly { readonly key: string; readonly module: FactPlace; readonly count: string }[]
  /** The declarations it opens wider, which breaks no use ("Opens wider:", `ApiChange.widened`), each at its declaration */
  readonly opensWider: FactList
  /**
   * The new uses between modules it adds ("New use between modules: view.gui.media → view.gui (38 links)"), each pair of
   * modules once, at the first use of its first link
   */
  readonly moduleEdges: FactList
  /**
   * Of the opening of a group of a module, its new uses between modules by their direction: the modules it now uses
   * ("Now uses view.gui (32 links) and shared/code") and those that now use it ("Now used by view.gui.media"), each at
   * the first use of its first link; absent elsewhere, where `moduleEdges` names the pairs
   */
  readonly uses?: FactList
  readonly usedBy?: FactList
  /**
   * Of a step none of whose code lands, the step of the later commit that took it out ("Nothing of it lands: step 5
   * took it out"): its words, the title of that commit in their hover, and the id of the step, which a click shows; null
   * otherwise, and for a step that lands nothing with no such commit known the line says "a later commit"
   */
  readonly takenOutBy: { readonly words: string; readonly title: string; readonly step: string | null } | null
  /**
   * Its units the section of another step shows (`ReviewStep.shownElsewhere`), by what it does to them and that step:
   * "Removes hooks, shown in step 2", each name at its place, a long list in the contents cut as every list is; its tests
   * unnamed, counted after the names ("Adds window and 2 tests", "Changes a test"); "step 2" showing that step
   */
  readonly shownElsewhere: readonly {
    readonly key: string
    readonly verb: string
    readonly names: FactList
    /** "2 tests", "a test": the tests among them, which a name of a sentence would not name; null for none, or counted in `names.more` */
    readonly tests: string | null
    readonly step: string
    readonly stepId: string
  }[]
  /**
   * accio answered, walked every way to the code it names, and no test of the change reaches any of its code and no
   * scenario proves it: "No test of the change reaches paint", each name at its place, a long list in the contents cut
   * as every list is
   */
  readonly unproven: { readonly names: FactList } | null
}

/** The names of a list in a sentence (`joinNames`): "run and create", "run, create, open and 2 more" */
const names = joinNames

/** The places of a list of facts the contents name before the count of the rest, as `joinNames` does */
const NAMED_FACTS = 3
/** The names of a fact the opening of a step names before "and N more": a list one longer is named whole */
export const OPENING_FACTS = 8

/** A list of the opening: up to `OPENING_FACTS` names and one more whole, of a longer one the first and the rest kept */
function openingList(places: readonly FactPlace[]): FactList {
  const { shown, more } = cutList(places, OPENING_FACTS)
  return more === 0 ? { places: shown, more } : { places: shown, more, rest: places.slice(shown.length) }
}

/** Every name of a list, its rest too */
function wholeList(list: FactList): readonly FactPlace[] {
  return list.rest === undefined ? list.places : [...list.places, ...list.rest]
}

/** The name of a file of a path: its last part */
function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1)
}

/**
 * Where the facts of a step stand: the contents of the overview, one short line a reader scans ("Tested by
 * ManagerTest.kt"); its opening, its first names and the rest on a click; or the opening of a group of it, as the
 * opening does, the new declarations of a new module by their names, since the group is that module
 */
export type FactsForm = "contents" | "opening" | "group"

/**
 * The hover of a test: its name, what of the step it reaches and through what ("joins two rows: calls Joiner", "opens:
 * reaches open through the test code", "opens: reaches open end to end")
 */
function testDetail(test: Pick<StepTest, "name" | "reaches" | "way">): string {
  const reaches = names(test.reaches)
  switch (test.way) {
    case "calls":
      return `${test.name}: calls ${reaches}`
    case "testCode":
      return `${test.name}: reaches ${reaches} through the test code`
    case "endToEnd":
      return `${test.name}: reaches ${reaches} end to end`
  }
}

/** Each file of tests of the change that reach a step (`StepTests::proof`) at its first test, its tests in its hover */
function proofPlaces(proof: ReviewStep["tests"]["proof"]): FactPlace[] {
  return proof.map(file => ({
    key: file.path,
    label: fileName(file.path),
    place: { path: file.path, side: "new" as const, line: file.tests[0]?.line ?? null, name: null },
    detail: file.tests.map(testDetail).join("\n"),
  }))
}

/**
 * The facts of `step` (`StepFacts`): the files of the tests of the change that reach its code, the scenarios that prove
 * it, its code no test of the change reaches beside proven code, the steps its tests prove, the public declarations it
 * adds and the links it adds between modules, and that nothing of it lands. In the contents (`form`) the tests are the files of the change's, and a long
 * list its first names and the count of the rest (`cutList`); the opening names up to `OPENING_FACTS` and one more, of a
 * longer list the first and "and N more", which shows the rest in place (`FactList.rest`). The warning of a step no test
 * reaches names its code in both
 */
export function presentStepFacts(step: ReviewStep, plan: Pick<ReviewPlan, "api" | "links" | "modules" | "steps" | "units" | "files">, form: FactsForm = "opening"): StepFacts {
  const { answered, proof, reached, unreached, scenarios } = step.tests
  const newApi = new Set(step.newApi)
  const addedChanges = plan.api.filter(change => change.kind === "added" && newApi.has(change.key))
  // a name two of them share is led by its owner, as the code of a step is: "Event.rightEdge and Update.rightEdge"
  const declared = (changes: readonly ApiChange[]): FactPlace[] =>
    changes.map(change => ({
      key: `${change.key}:${change.path}:${change.line}`,
      label: changes.filter(other => other.name === change.name).length > 1 ? ownerLed(change.key, change.name) : change.name,
      place: { path: change.path, side: "new" as const, line: change.line, name: change.name },
    }))
  // the new declarations of a module the change adds, by that module; the opening of its group names them
  const moduleOf = new Map(plan.files.map(file => [file.path, file.module]))
  const newModuleOf = (change: ApiChange): number | null => {
    const module = moduleOf.get(change.path) ?? null
    return form !== "group" && module !== null && plan.modules[module]?.state === "added" ? module : null
  }
  const fresh = addedChanges.filter(change => !change.widened)
  const ofNewModules = new Map<number, ApiChange[]>()
  for (const change of fresh) {
    const module = newModuleOf(change)
    if (module !== null) {
      ofNewModules.set(module, [...(ofNewModules.get(module) ?? []), change])
    }
  }
  const edges = step.moduleEdges.flatMap(edge => {
    const [from, to, link] = [plan.modules[edge.from], plan.modules[edge.to], plan.links[edge.links[0] ?? -1]]
    if (from === undefined || to === undefined || link === undefined) {
      return []
    }
    const count = edge.links.length
    return [{ key: `${edge.from}:${edge.to}`, label: `${from.label} → ${to.label}${count > 1 ? ` (${count} links)` : ""}`, place: firstUse(link) }]
  })
  const takenOutBy = takenOutOf(step, plan)
  const contents = form === "contents"
  // the code no test reaches, beside code a test reaches or a scenario proves; alone it is the warning (`unproven`)
  const noneBeside = unreached.length > 0 && (contents ? reached === 0 && scenarios.length > 0 : reached > 0 || scenarios.length > 0)
  // the contents name a long list as every list does; the opening names its first names and the rest on a click
  const list = (places: readonly FactPlace[]): FactList => {
    if (!contents) {
      return openingList(places)
    }
    const { shown, more } = cutList(places, NAMED_FACTS)
    return { places: shown, more }
  }
  const shownElsewhere = shownElsewhereOf(step, plan, list)
  const noneFact = (): TestFact => ({ key: "none", words: "No test of the change reaches", names: list(unreached.map(declarationPlace)) })
  const changeFact: TestFact | null = proof.length === 0 ? null : { key: "change", words: "Tested by", names: list(proofPlaces(proof)) }
  const tests: TestFact[] = [changeFact, noneBeside ? noneFact() : null].filter(fact => fact !== null)
  const parts = [
    scenarios.length === 0 ? null : `Proven by ${names(scenarios.map(fileName))}`,
    step.stops.length === 0 && step.shownElsewhere.length === 0 && takenOutBy === null ? "Nothing of it lands: a later commit took it out" : null,
  ].filter(part => part !== null)
  return {
    line: parts.length === 0 ? null : parts.join(" · "),
    tests,
    proves: step.proves.flatMap(index => {
      const proved = plan.steps.steps[index]
      return proved === undefined ? [] : [{ key: proved.id, step: `step ${index + 1}`, stepId: proved.id }]
    }),
    newApi: list(declared(fresh.filter(change => newModuleOf(change) === null))),
    newModules: [...ofNewModules].map(([module, changes]) => ({
      key: `${module}`,
      module: { key: `${module}`, label: plan.modules[module]?.label ?? "", place: declared(changes)[0]!.place, detail: changes.map(change => change.name).join(", ") },
      count: changes.length === 1 ? "1 declaration" : `${changes.length} declarations`,
    })),
    opensWider: list(declared(addedChanges.filter(change => change.widened))),
    // the contents leave to the map the new uses between modules, and to the opening the units shown in another step
    moduleEdges: contents ? { places: [], more: 0 } : list(edges),
    takenOutBy,
    shownElsewhere: contents ? [] : shownElsewhere,
    unproven: answered && unreached.length > 0 && reached === 0 && scenarios.length === 0 ? { names: noneFact().names } : null,
  }
}

/** A declaration of a step at the line of its name at the head, the name selected; a name led by its owner selects the name itself */
function declarationPlace(declaration: { readonly name: string; readonly path: string; readonly line: number }): FactPlace {
  return {
    key: `${declaration.path}:${declaration.line}:${declaration.name}`,
    label: declaration.name,
    place: { path: declaration.path, side: "new" as const, line: declaration.line, name: declaration.name.slice(declaration.name.lastIndexOf(".") + 1) },
  }
}

/**
 * The name of a declaration led by its owner, the type before it in its key ("a.Event.rightEdge" → "Event.rightEdge"),
 * as Rust leads the code of a step (`steps::code_of`); the name alone in no type (a package is no owner)
 */
function ownerLed(key: string, name: string): string {
  const containers = key.endsWith(`.${name}`) ? key.slice(0, -name.length - 1) : ""
  const owner = containers.slice(containers.lastIndexOf(".") + 1)
  return /^[A-Z]/.test(owner) ? `${owner}.${name}` : name
}

/** What a step does to a unit of its own that another step shows: "Removes", "Adds" */
function verbOf(unit: ReviewUnit | undefined): string {
  switch (unit?.kind.kind) {
    case "added":
    case "newFile":
      return "Adds"
    case "removed":
    case "deletedFile":
    case "deletedFiles":
      return "Removes"
    default:
      return "Changes"
  }
}

/**
 * The units of `step` that a section of another step shows, by what it does to them and that step, in their order:
 * each name at its place, a declaration at the line of its name (a removed one before the commits), a file at its top,
 * the names cut as `list` cuts them; the tests unnamed, counted after the names, or in the count of the rest of a cut
 * list. A Kotlin test is named by a sentence, which reads as no code
 */
function shownElsewhereOf(step: ReviewStep, plan: Pick<ReviewPlan, "steps" | "units">, list: (places: readonly FactPlace[]) => FactList): StepFacts["shownElsewhere"] {
  const units = new Map(plan.units.map(unit => [unit.id, unit]))
  const groups: { key: string; verb: string; places: FactPlace[]; tests: number; step: number }[] = []
  for (const shown of step.shownElsewhere) {
    const unit = units.get(shown.unit)
    if (unit === undefined) {
      continue
    }
    const verb = verbOf(unit)
    let group = groups.find(it => it.verb === verb && it.step === shown.step)
    if (group === undefined) {
      group = { key: `${verb}:${shown.step}`, verb, places: [], tests: 0, step: shown.step }
      groups.push(group)
    }
    if (unit.test) {
      group.tests++
    } else {
      group.places.push(unitPlace(unit))
    }
  }
  return groups.flatMap(group => {
    const holder = plan.steps.steps[group.step]
    if (holder === undefined) {
      return []
    }
    const names = list(group.places)
    // a cut list counts its tests with the rest of it: "a, b, c and 4 more"
    const counted = names.more > 0 && group.tests > 0
    return [
      {
        key: group.key,
        verb: group.verb,
        names: counted ? { ...names, more: names.more + group.tests } : names,
        tests: counted || group.tests === 0 ? null : group.tests === 1 ? "a test" : `${group.tests} tests`,
        step: `step ${group.step + 1}`,
        stepId: holder.id,
      },
    ]
  })
}

/**
 * A unit by its name at its place: a declaration at the line of its name, a removed one before the commits, at the path
 * its file had then; a file at its top
 */
function unitPlace(unit: ReviewUnit): FactPlace {
  const slice = unit.slices[0]
  const removed = unit.kind.kind === "removed" || unit.kind.kind === "deletedFile" || unit.kind.kind === "deletedFiles"
  const path = (removed ? slice?.oldPath : null) ?? slice?.path ?? ""
  const name = unit.name ?? fileName(path)
  return {
    key: unit.id,
    label: name,
    place: unit.key === null ? fileTop(path) : { path, side: removed ? "old" : "new", line: unit.line, name },
  }
}

/**
 * The commit that took out the code of `step` (`ReviewStep.takenOutBy`) by the step it is in ("step 5"), its title for
 * the hover: a subject of a hundred characters inside a sentence does not read
 */
function takenOutOf(step: ReviewStep, plan: Pick<ReviewPlan, "steps">): StepFacts["takenOutBy"] {
  const id = step.stops.length === 0 ? step.takenOutBy : null
  if (id === null) {
    return null
  }
  const place = plan.steps.steps.findIndex(it => it.commits.some(commit => commit.id === id))
  const holder = plan.steps.steps[place]
  const title = holder?.commits.find(commit => commit.id === id)?.title ?? plan.steps.commits.find(commit => commit.id === id)?.title
  if (title === undefined) {
    return null
  }
  return { words: holder === undefined ? "a later commit" : `step ${place + 1}`, title, step: holder?.id ?? null }
}

/** A step in the contents of the overview */
export interface ContentsStep {
  readonly id: string
  readonly number: number | null
  /** Its full title: the subject of the commit that starts it without the issue key, a concern by its name */
  readonly title: string
  /** Its title is an identifier: set as code */
  readonly code: boolean
  /** The summary its commit's body opens with, with its code names; null without */
  readonly lead: { readonly story: StoryView; readonly block: PlacedBlock } | null
  readonly facts: StepFacts
  /**
   * The sections of its story where the author asks the reviewer to decide ("Look closely:"), each its first block:
   * of the commit that starts it, then of each that follows it up
   */
  readonly lookClosely: readonly { readonly story: StoryView; readonly part: BodyPart }[]
}

/** A file of What Changes: its kind and path once, then each section of it the change touches */
export interface ContractFile {
  readonly key: string
  readonly path: string
  /** "Spec rule", "Decision", "Agent guide", "Doc" */
  readonly kind: string
  /**
   * "7 changes in User Experience and Requirements": the line of a file whose quotes the opening folds; "New spec (9
   * requirements)" of a spec the change adds, "Deleted" of a file it deletes
   */
  readonly summary: string
  /** Its sections, each its heading once with the paragraphs the change adds, removes or changes under it */
  readonly entries: readonly {
    readonly key: string
    /** The heading of its section of Markdown */
    readonly heading: string | null
    /** Its paragraphs, the rows of one table one block */
    readonly blocks: readonly ContractBlock[]
    /** "Proven by restart.json" */
    readonly provenBy: string | null
  }[]
}

/** A block of a section of What Changes: a paragraph, or the rows of one table the change touches */
export type ContractBlock = { readonly kind: "paragraph"; readonly paragraph: ContractParagraph } | ContractTable

/**
 * The rows of one table of What Changes the change adds, removes or changes (`ProseForm::Row`), in their order, under
 * their header once: the header row itself when the change touched it (its cells with their marks), else the cells of
 * the header of its first row
 */
export interface ContractTable {
  readonly kind: "table"
  readonly key: string
  /** The header row the change adds, removes or changes, `null` when it kept it */
  readonly head: ContractParagraph | null
  readonly rows: readonly ContractParagraph[]
}

/**
 * A paragraph of What Changes, whole (`ProseParagraph`): its text with its spans of code and its links as Rust found
 * them, what the change did to it (`null` for a note of the brief: "Deleted", the description of a new spec without
 * one), and of a long one changed in a few places the sentences shown until it opens whole
 */
export interface ContractParagraph {
  readonly key: string
  readonly text: string
  readonly form: ProseForm
  readonly code: readonly CodeSpan[]
  readonly links: readonly ProseLinkView[]
  readonly substitutions: readonly { readonly start: number; readonly end: number; readonly from: string }[]
  readonly change: ProseChange | null
  readonly excerpt: TextRange | null
}

/** A repeated word replacement named once above What Changes, with its files and their places when the review has them */
export interface SubstitutionView {
  readonly key: string
  readonly from: string
  readonly to: string
  readonly summary: string
  readonly files: readonly { readonly path: string; readonly name: string; readonly place: ReviewPlace | null }[]
  readonly allFiles: string
}

/**
 * A link of a paragraph of prose: the range of its text and where a click goes, a changed file of the review at its top
 * (`place`) or a page (`href`); neither for a file the review does not show, which stays text
 */
export interface ProseLinkView {
  readonly start: number
  readonly end: number
  readonly place: ReviewPlace | null
  readonly href: string | null
}

/** A commit the overview says was rewritten or reworded since the review */
export interface SinceCommit {
  readonly key: string
  /** "Rewritten scale the helper: src/Store.kt +1 −1 · message changed" */
  readonly text: string
  /** Its subject whole, in the native hover */
  readonly title: string
  /** How its message changed (`PairedCommit.messageChange`), the blocks the line opens to; null when it did not */
  readonly message: MessageChange | null
}

/** The overview at the top of the document, worded */
export interface PrologueView {
  /** The issue key the commits name, a chip beside the headline; null without one */
  readonly issue: string | null
  /** The headline Rust names (`ReviewPlan.title`): the branch as the title bar names it, else the range */
  readonly title: string
  /**
   * Of the steps of the concerns of one commit, its story told before the steps: the lead and what names nothing of a
   * step; null otherwise
   */
  readonly untold: { readonly subject: string; readonly story: StoryView } | null
  /** The shape of the change, its contract in one line and the API it opens wider (`presentMap`) */
  readonly map: MapView
  /**
   * What Changes whole: of a review whose steps are not told; with steps each entry is quoted in the opening of the
   * step that changes it (`OpeningView.contract`) and the map names the contract in one line
   */
  readonly contract: readonly ContractFile[]
  /** The mechanical replacements shared by the paragraphs of What Changes */
  readonly substitutions: readonly SubstitutionView[]
  /** The contents: the steps of the story; none when the only step is what no evidence joined */
  readonly steps: readonly ContentsStep[]
  /** Why the tasks of the range are its steps, a muted line under the label of the steps: "One commit: the steps are its tasks" */
  readonly source: string | null
  /** Without a step in the contents, the facts of the range: the new API, the new links between modules; null with steps */
  readonly facts: StepFacts | null
  readonly breakage: Breakage
  /**
   * "Since your review: 1 new · 1 rewritten" and the line of each rewritten or reworded commit, with how its message
   * changed when it did, which the line opens; null without a change since
   */
  readonly since: { readonly summary: string; readonly rewritten: readonly SinceCommit[] } | null
  /**
   * Why accio did not answer, one line above the map ("accio has no index of this repository"): without it the
   * review knows no use left outside and no test of a step, so the whole review is less, not one entry of it; null when
   * it answered
   */
  readonly indexNote: string | null
}

const CONTRACT_WORDS: Readonly<Record<ContractKind, string>> = {
  spec: "Spec rule",
  decision: "Decision",
  guide: "Agent guide",
  doc: "Doc",
}

/**
 * The parts of `story` where the author asks the reviewer to decide, each its question alone (its first block): the
 * opening tells the rest
 */
function lookCloselyOf(story: StoryView): ContentsStep["lookClosely"] {
  return story.shown.filter(part => part.tone === "lookClosely").map(part => ({ story, part: { ...part, blocks: part.blocks.slice(0, 1) } }))
}

/** The stories of the commits that follow `step` up */
function followUpStories(step: ReviewStep, plan: Pick<ReviewPlan, "steps">, words: MentionWords): StoryView[] {
  return step.commits.flatMap(commit => {
    const story = commit.role === "followsUp" ? storyOf(plan, commit.id) : undefined
    return story === undefined ? [] : [presentStory(story, undefined, words)]
  })
}

/** The story a step's commit tells: none for a commit of no story, whose few words tell no step */
function storyOf(plan: Pick<ReviewPlan, "steps">, id: string | undefined): StoryCommit | undefined {
  return id === undefined ? undefined : plan.steps.commits.find(it => it.id === id)
}

/**
 * The overview of `plan` (`PrologueView`), in the order Rust gave it: the headline, the story of a commit told before
 * the steps of its concerns, What Changes one header per file, the contents of the steps, Possible Breakage, what
 * changed since the review; `pending`: Rust asks the index for the usages, the plan with them follows
 */
export function presentPrologue(plan: ReviewPlan, pending: boolean): PrologueView {
  const chapters = presentChapters(plan)
  const words = mentionWords(plan)
  const told = chapters.some(chapter => chapter.told)
  const one = storyByParts(plan)
  const steps = (told ? plan.steps.steps : []).map((step, index): ContentsStep => {
    const chapter = chapters[index]!
    const head = headOf(step)
    const byParts = tellsParts(plan, step)
    const story = byParts ? undefined : head?.role === "noStory" ? undefined : storyOf(plan, head?.id)
    const view = story === undefined ? null : presentStory(story, undefined, words)
    const toldHere = one === null || !byParts ? null : presentStory(one, part => toldInStep(part, index), words)
    return {
      id: step.id,
      number: chapter.rest ? null : chapter.number,
      title: chapter.label,
      code: chapter.code,
      lead: view?.lead == null ? null : { story: view, block: view.lead },
      facts: presentStepFacts(step, plan, "contents"),
      // the author's question of a round too: its story is folded under the step's opening
      lookClosely: view !== null ? [view, ...followUpStories(step, plan, words)].flatMap(lookCloselyOf) : toldHere === null ? [] : lookCloselyOf(toldHere),
    }
  })
  const since = plan.interdiff === null ? null : formatSinceReview(plan.interdiff)
  const contract = told ? [] : presentContract(plan)
  // a repeated replacement is of the whole change: named once in the overview, its paragraphs quiet where they are quoted
  const substitutions = presentSubstitutions(plan)
  const breakage = presentBreakage(plan.api, pending, plan.verdict)
  const map = presentMap(plan)
  return {
    issue: plan.brief.issue,
    title: plan.title,
    // without a step of a task the diffs have no opening: the whole story is told here
    untold: one === null ? null : { subject: one.title, story: presentStory(one, part => part.told.kind === "overview", words) },
    map,
    contract,
    substitutions,
    steps,
    source: told ? formatStepSource(plan.steps.source) : null,
    facts: told ? null : rangeFacts(plan),
    // the API none of whose uses breaks is said once: under the verdict on the line of the map when nothing can break
    breakage: map.api?.verdict == null ? breakage : { ...breakage, unbroken: null },
    since:
      since === null || plan.interdiff === null
        ? null
        : {
            summary: `Since your review: ${since}`,
            rewritten: plan.interdiff.commits
              .filter(commit => (commit.state === "rewritten" && commit.changes !== null) || (commit.state === "reworded" && commit.messageChange !== null))
              .map((commit, index) => ({
                key: `${commit.id ?? index}`,
                text: `${formatPairState(commit.state)} ${commit.subject}: ${formatRewrittenChanges(commit)}`,
                title: commit.subject,
                message: commit.messageChange,
              })),
          },
    // Rust's note names accio and says why ("accio has no index of this repository"); the line adds what it costs
    indexNote: plan.indexNote,
  }
}

/** A note of the brief in place of paragraphs: of a spec the commits add, of a file they delete */
function contractNote(key: string, text: string): ContractParagraph {
  return { key, text, form: { kind: "prose" }, code: [], links: [], substitutions: [], change: null, excerpt: null }
}

/**
 * What Changes of `plan` (`ReviewBrief.contract`): one header per file, under it each section the change touches, its
 * heading once (the entries of one section, which Rust tells apart by their blocks, one), its paragraphs whole; a link
 * of a paragraph shows a changed file of the review or opens a page
 */
function presentContract(plan: Pick<ReviewPlan, "brief" | "files">, keep: (entry: ContractEntry, index: number) => boolean = () => true): ContractFile[] {
  const shown = new Set(plan.files.flatMap(file => (file.oldPath === null ? [file.path] : [file.path, file.oldPath])))
  const files: {
    key: string
    path: string
    kind: string
    // the line of a file the change adds as a spec or deletes, whose one note counts no change
    whole: string | null
    entries: { key: string; heading: string | null; paragraphs: ContractParagraph[]; provenBy: string[] }[]
  }[] = []
  plan.brief.contract.forEach((entry, index) => {
    if (!keep(entry, index)) {
      return
    }
    const key = `${entry.unit}:${entry.blocks[0] ?? "file"}:${index}`
    const paragraphs =
      entry.newSpec !== null
        ? [
            entry.newSpec.description === null
              ? contractNote(`${key}:0`, `A new spec of ${entry.newSpec.rules} rules`)
              : { ...contractNote(`${key}:0`, entry.newSpec.description), change: { kind: "added" as const } },
          ]
        : entry.deleted
          ? [contractNote(`${key}:0`, "Deleted")]
          : entry.paragraphs.map((paragraph, at): ContractParagraph => ({
              key: `${key}:${at}`,
              text: paragraph.text,
              form: paragraph.form,
              code: paragraph.code,
              links: paragraph.links.map(link => ({
                start: link.start,
                end: link.end,
                place: shown.has(link.target) ? fileTop(link.target) : null,
                href: link.target.includes("://") || link.target.startsWith("mailto:") ? link.target : null,
              })),
              substitutions:
                paragraph.change.kind === "changed" || paragraph.change.kind === "substituted"
                  ? paragraph.change.substitutions.flatMap(mark => {
                      const substitution = plan.brief.substitutions[mark.substitution]
                      return substitution === undefined ? [] : [{ start: mark.start, end: mark.end, from: substitution.from }]
                    })
                  : [],
              change: paragraph.change,
              excerpt: paragraph.excerpt,
            }))
    let file = files.at(-1)
    if (file?.path !== entry.path) {
      file = { key, path: entry.path, kind: CONTRACT_WORDS[entry.kind], whole: null, entries: [] }
      files.push(file)
    }
    if (entry.newSpec !== null) {
      file.whole = `New spec (${entry.newSpec.rules === 1 ? "1 requirement" : `${entry.newSpec.rules} requirements`})`
    } else if (entry.deleted) {
      file.whole = "Deleted"
    }
    const section = file.entries.at(-1)
    if (section !== undefined && section.heading === entry.heading) {
      section.paragraphs.push(...paragraphs)
      section.provenBy.push(...entry.provenBy.filter(test => !section.provenBy.includes(test)))
    } else {
      file.entries.push({ key, heading: entry.heading, paragraphs, provenBy: [...entry.provenBy] })
    }
  })
  return files.map(({ whole, ...file }) => ({
    ...file,
    summary: whole ?? contractSummary(file.entries),
    entries: file.entries.map(entry => ({
      key: entry.key,
      heading: entry.heading,
      blocks: contractBlocks(entry.paragraphs),
      provenBy: entry.provenBy.length === 0 ? null : `Proven by ${names(entry.provenBy.map(fileName))}`,
    })),
  }))
}

/** The words of the cells `ranges` of a row of a table, one string to tell one header from another */
function cellWords(row: ContractParagraph, ranges: readonly TextRange[]): string {
  return ranges.map(range => row.text.slice(range.start, range.end)).join("\u0000")
}

/**
 * The paragraphs of a section as its blocks: a run of rows of one table one block, the rows under one header together;
 * the header row itself (a row of no header) heads the rows whose header it is
 */
function contractBlocks(paragraphs: readonly ContractParagraph[]): ContractBlock[] {
  const blocks: ContractBlock[] = []
  let table: { key: string; head: ContractParagraph | null; header: string; rows: ContractParagraph[] } | null = null
  for (const paragraph of paragraphs) {
    const form = paragraph.form
    if (form.kind !== "row") {
      table = null
      blocks.push({ kind: "paragraph", paragraph })
      continue
    }
    if (form.header.length === 0) {
      table = { key: paragraph.key, head: paragraph, header: cellWords(paragraph, form.cells), rows: [] }
      blocks.push({ kind: "table", key: table.key, head: paragraph, rows: table.rows })
      continue
    }
    const header = cellWords(paragraph, form.header)
    if (table?.header !== header) {
      table = { key: paragraph.key, head: null, header, rows: [] }
      blocks.push({ kind: "table", key: table.key, head: null, rows: table.rows })
    }
    table.rows.push(paragraph)
  }
  return blocks
}

/** The repeated replacements the changed paragraphs share, with names for the row and places only for changed files */
function presentSubstitutions(plan: Pick<ReviewPlan, "brief" | "files">): SubstitutionView[] {
  const changedPaths = new Map<string, string>()
  for (const file of plan.files) {
    changedPaths.set(file.path, file.path)
    if (file.oldPath !== null) {
      changedPaths.set(file.oldPath, file.path)
    }
  }
  return plan.brief.substitutions.map((substitution, index) => {
    const files = substitution.paths.map(path => {
      const target = changedPaths.get(path)
      return { path, name: fileName(path), place: target === undefined ? null : fileTop(target) }
    })
    const places = `${substitution.places} ${substitution.places === 1 ? "place" : "places"}`
    const count = `${files.length} ${files.length === 1 ? "file" : "files"}`
    return {
      key: `${index}:${substitution.from}:${substitution.to}`,
      from: substitution.from,
      to: substitution.to,
      summary: `${places} in ${count}`,
      files,
      allFiles: substitution.paths.join("\n"),
    }
  })
}

/** The facts of every step of `plan` as one, each list cut as the opening cuts one: of a range whose contents list no step; null without a fact */
function rangeFacts(plan: ReviewPlan): StepFacts | null {
  const all = plan.steps.steps.map(step => presentStepFacts(step, plan))
  const lines = all.flatMap(facts => (facts.line === null ? [] : [facts.line]))
  // no step opens: every name is here
  const facts = {
    line: lines.length === 0 ? null : lines.join(" · "),
    tests: all.flatMap(it => it.tests),
    newApi: openingList(all.flatMap(it => wholeList(it.newApi))),
    newModules: all.flatMap(it => it.newModules),
    opensWider: openingList(all.flatMap(it => wholeList(it.opensWider))),
    moduleEdges: openingList(all.flatMap(it => wholeList(it.moduleEdges))),
    proves: [],
    takenOutBy: null,
    shownElsewhere: [],
    unproven: null,
  }
  const named = [facts.newApi, facts.opensWider, facts.moduleEdges].some(it => it.places.length > 0) || facts.newModules.length > 0
  return facts.line === null && facts.tests.length === 0 && !named ? null : facts
}

/** A commit of a step after the one that starts it, in its opening: one row each, the same form */
export interface OpeningCommit {
  readonly id: string
  /** "Follows up", "Without a story": muted before its title */
  readonly role: string
  /** Its title, as Rust gives it: its subject without the issue key */
  readonly title: string
  /** Its story, which its disclosure opens; null for a commit of no story, which has no chevron */
  readonly story: StoryView | null
}

/** The opening of a step above the card of its first section, worded */
export interface OpeningView {
  readonly id: string
  /** "Step 2", null for "Other changes" */
  readonly number: string | null
  /** Its full title, which wraps */
  readonly title: string
  /** Its title is an identifier: set as code */
  readonly code: boolean
  /** Its title is the first sentence of the paragraph it tells, its lead here: no title line of its own (`Chapter.titleInBody`) */
  readonly titleInBody: boolean
  /** Of a step of the commits: the subject the commit that starts it had at the review, when a reword changed it */
  readonly subjectBefore: string | null
  /** Of a step of a concern: what joins its parts ("Joined by the identifiers" `hide`, `show`), in the line of its facts */
  readonly evidence: readonly EvidencePart[]
  /** The story it tells: the body of the commit that starts it, of a concern of one commit the blocks told in it */
  readonly story: StoryView | null
  /** The commits after the one that starts it: those that follow it up, then those of no story whose lines are mostly in it */
  readonly commits: readonly OpeningCommit[]
  readonly facts: StepFacts
  /**
   * The entries of the brief the step changes, quoted under its story (What Changes of the step): each entry in the
   * first step whose stops show its unit and one of its blocks
   */
  readonly contract: readonly ContractFile[]
}

/** The opening of each step of `plan` by its id (`OpeningView`) */
export function presentOpenings(plan: Pick<ReviewPlan, "steps" | "concerns" | "api" | "links" | "modules" | "units" | "brief" | "files">): Map<string, OpeningView> {
  const chapters = presentChapters(plan)
  const stepOfEntry = contractSteps(plan)
  const concerns = new Map(plan.concerns.map(concern => [concern.id, concern]))
  const one = storyByParts(plan)
  return new Map(
    plan.steps.steps.map((step, index): [string, OpeningView] => {
      // the names of a story told in this step: their step is said only when it is another
      const words = mentionWords(plan, index)
      const commitOf = (id: string, title: string, role: CommitRole): OpeningCommit => {
        const story = role === "noStory" ? undefined : storyOf(plan, id)
        return { id, role: role === "followsUp" ? "Follows up" : "Without a story", title, story: story === undefined ? null : presentStory(story, undefined, words) }
      }
      const chapter = chapters[index]!
      const concern = step.concern === null ? undefined : concerns.get(step.concern)
      const byParts = tellsParts(plan, step)
      const head = byParts ? undefined : headOf(step)
      const starts = head?.role === "starts" ? head : undefined
      const story = byParts
        ? one === null
          ? null
          : presentStory(one, part => toldInStep(part, index), words, chapter.titleInBody)
        : starts === undefined
          ? null
          : commitOf(starts.id, starts.title, "starts").story
      const rest = byParts ? [] : step.commits.filter(commit => commit !== starts)
      return [
        step.id,
        {
          id: step.id,
          number: chapter.rest ? null : formatStepNumber(chapter.number),
          title: chapter.label,
          code: chapter.code,
          titleInBody: chapter.titleInBody && story?.lead != null,
          subjectBefore: byParts ? null : (story?.subjectBefore ?? null),
          evidence: concern === undefined ? [] : presentConcernEvidence(concern.evidence),
          story: story === null || (story.lead === null && story.shown.length === 0 && story.folded.length === 0) ? null : story,
          commits: [...rest.filter(commit => commit.role === "followsUp"), ...rest.filter(commit => commit.role === "noStory")].map(commit =>
            commitOf(commit.id, commit.title, commit.role),
          ),
          facts: step.groups.some(opensGroup) ? withoutGroupFacts(presentStepFacts(step, plan)) : presentStepFacts(step, plan),
          contract: plan.steps.told ? presentContract(plan, (_, entry) => stepOfEntry[entry] === index) : [],
        },
      ]
    }),
  )
}

const NO_FACTS: FactList = { places: [], more: 0 }

/**
 * The facts of a step whose groups open: what the groups say of their code (its tests and its code none reaches, its new
 * API, what it opens wider, its new uses between modules) leaves the opening of the step (`presentGroupOpenings`)
 */
function withoutGroupFacts(facts: StepFacts): StepFacts {
  return { ...facts, tests: [], newApi: NO_FACTS, newModules: [], opensWider: NO_FACTS, moduleEdges: NO_FACTS, unproven: null }
}

/**
 * The opening of a group of a large step (spec/branch/review-tour.spec.md, "In the Review Window"): above the first card
 * of the group, in place of its divider, what the group is and what its code does to the rest
 */
export interface GroupOpeningView {
  /** `groupOpeningKey` of its step and its place */
  readonly key: string
  /** The label of its module ("view.gui.media"), of a group of a folder the folder */
  readonly name: string
  /** A module's label is set in the code font */
  readonly code: boolean
  /** "New module", "Deleted module"; null for a module the change only changes and for a folder */
  readonly state: string | null
  /** "also shared/code": the small modules it holds */
  readonly also: string | null
  /** The root of its module: the hover of its name */
  readonly root: string | null
  /**
   * Its facts as a step's (`presentStepFacts`, the form "group"): the tests of the change that reach the code of its
   * files and its code none reaches, its new API by name, what it opens wider, its new uses between modules
   */
  readonly facts: StepFacts
  /** What its module registers in the platform, the first `OPENING_FACTS` (a group of a module, not of a folder) */
  readonly registrations: readonly MapRegistration[]
  readonly moreRegistrations: number
  /** The texts of the UI its module adds, changes or removes (a group of a module) */
  readonly texts: MapTexts | null
}

/** The opening of each group of a large step that opens (`opensGroup`), by `groupOpeningKey` */
export function presentGroupOpenings(plan: Pick<ReviewPlan, "steps" | "api" | "links" | "modules" | "units" | "files" | "registrations" | "texts">): Map<string, GroupOpeningView> {
  const openings = new Map<string, GroupOpeningView>()
  for (const step of plan.steps.steps) {
    step.groups.forEach((group, place) => {
      if (!opensGroup(group)) {
        return
      }
      const stops = step.stops.slice(group.start, group.end)
      const paths = new Set(stops.map(stop => stop.path))
      const modules = group.module === null ? [] : [group.module, ...group.also]
      // the step as its group reads it: its stops, its tests, the new API of its files, the uses between modules of its own
      const ofGroup: ReviewStep = {
        ...step,
        stops,
        tests: group.tested,
        newApi: step.newApi.filter(key => plan.api.some(change => change.key === key && paths.has(change.path))),
        moduleEdges: [],
        shownElsewhere: [],
        proves: [],
      }
      // its new uses between modules by their direction: of its modules, then of the others of it
      const edge = (other: number, links: readonly number[]): FactPlace[] => {
        const [label, link] = [plan.modules[other]?.label, plan.links[links[0] ?? -1]]
        return label === undefined || link === undefined
          ? []
          : [{ key: `${other}:${links[0]}`, label: links.length > 1 ? `${label} (${links.length} links)` : label, place: firstUse(link) }]
      }
      const uses = step.moduleEdges.flatMap(it => (modules.includes(it.from) ? edge(it.to, it.links) : []))
      const usedBy = step.moduleEdges.flatMap(it => (!modules.includes(it.from) && modules.includes(it.to) ? edge(it.from, it.links) : []))
      const module = group.module === null ? undefined : plan.modules[group.module]
      const ofModule = group.kind === "module" && group.module !== null
      const { registrations, more } = ofModule ? moduleRegistrations(plan, group.module!, OPENING_FACTS) : { registrations: [], more: 0 }
      const key = groupOpeningKey(step.id, place)
      openings.set(key, {
        key,
        name: group.kind === "folder" && group.folder !== null && group.folder !== "" ? group.folder : (module?.label ?? ""),
        code: group.kind === "module",
        state: module === undefined || module.state === "changed" || group.kind === "folder" ? null : formatModuleState(module.state),
        also: alsoOf(group.also, plan.modules),
        root: module?.root ?? null,
        facts: { ...presentStepFacts(ofGroup, plan, "group"), takenOutBy: null, uses: openingList(uses), usedBy: openingList(usedBy) },
        registrations,
        moreRegistrations: more,
        texts: ofModule ? textsOf(plan.texts.filter(text => text.module === group.module)) : null,
      })
    })
  }
  return openings
}

/** "7 changes in User Experience and Requirements": how much of a file the change quotes, by its sections */
function contractSummary(entries: readonly { readonly heading: string | null; readonly paragraphs: readonly unknown[] }[]): string {
  const count = entries.reduce((sum, entry) => sum + entry.paragraphs.length, 0)
  const headings = [...new Set(entries.flatMap(entry => (entry.heading === null ? [] : [entry.heading])))]
  const changes = count === 1 ? "1 change" : `${count} changes`
  return headings.length === 0 ? changes : `${changes} in ${joinNames(headings)}`
}

/** The step of each entry of the brief, by its place: the first whose stops show its unit and one of its blocks, else -1 */
function contractSteps(plan: Pick<ReviewPlan, "steps" | "brief">): number[] {
  return plan.brief.contract.map(entry =>
    plan.steps.steps.findIndex(step =>
      step.stops.some(stop => stop.unit === entry.unit && (stop.blocks === null || entry.blocks.length === 0 || entry.blocks.some(block => stop.blocks!.includes(block)))),
    ),
  )
}

/**
 * A mark of a section of the diffs that a body names (spec/branch/review-tour.spec.md, "Anchors"): the block of the
 * story that names it, where that block is told (the opening of a step, else the overview) and its words
 */
export interface StoryMark {
  readonly key: string
  readonly commit: string
  /** The block of `CommitBody.blocks` that names the section */
  readonly block: number
  /** The step whose opening tells the block, by its id; null for the overview */
  readonly step: string | null
  /** Where it is told: "Step 2 · The open keeps a stored layout", "Overview" */
  readonly title: string
  /** The place of `title` ("Step 2", "Overview") and the label of its step after it, `null` for the overview */
  readonly place: string
  readonly label: string | null
  /** The label of its step is an identifier, set as code (`Chapter.code`) */
  readonly code: boolean
  /** The text of the line of the block that names the section, its backticks kept */
  readonly preview: string
  /** The spans of code of that line, as Rust found them */
  readonly spans: readonly CodeSpan[]
  /** The code names of that line, worded (`mentionWords`): the quote of a mark links them as the story does */
  readonly mentions: readonly StoryMention[]
}

/**
 * The text of the line `item` of a block: a line of a paragraph, an item of a list, a cell of a table (the header row
 * first, then row by row), a heading or a label
 */
function lineOf(block: CommitBodyBlock | undefined, item: number): string {
  switch (block?.type) {
    case "paragraph":
      return block.lines[item] ?? ""
    case "list":
      return block.items[item] ?? ""
    case "table":
      return [...block.header, ...block.rows.flat()][item] ?? ""
    case "heading":
    case "label":
      return block.text
    default:
      return ""
  }
}

/**
 * The marks of each section of the story by its key, worded: the lines of the told bodies that name a stop of the
 * section, as Rust found them (`StepStop.marks`, `NarrativeSection.story`), one per block that names it, each where it
 * is told (the opening of a step, else the overview)
 */
export function storyMarks(plan: Pick<ReviewPlan, "steps" | "concerns" | "units">, sections: readonly NarrativeSection[]): Map<string, StoryMark[]> {
  const chapters = presentChapters(plan)
  const words = mentionWords(plan)
  const steps = plan.steps.steps
  // worded once per commit: a mark quotes the worded mentions of its line
  const worded = new Map<string, StoryMention[]>()
  const marks = new Map<string, StoryMark[]>()
  for (const section of sections) {
    for (const ref of section.story) {
      const commit = plan.steps.commits.find(it => it.id === ref.commit)
      if (commit === undefined) {
        continue
      }
      let mentions = worded.get(commit.id)
      if (mentions === undefined) {
        mentions = wordMentions(commit, words)
        worded.set(commit.id, mentions)
      }
      const chapter = ref.step === null ? undefined : chapters[ref.step]
      const list = marks.get(section.key) ?? []
      if (!list.some(mark => mark.commit === ref.commit && mark.block === ref.block)) {
        list.push({
          key: `${ref.commit}:${ref.block}`,
          commit: ref.commit,
          block: ref.block,
          step: ref.step === null ? null : (steps[ref.step]?.id ?? null),
          title: chapter === undefined ? "Overview" : `${formatStepNumber(chapter.number)} · ${chapter.label}`,
          place: chapter === undefined ? "Overview" : formatStepNumber(chapter.number),
          label: chapter?.label ?? null,
          code: chapter?.code ?? false,
          preview: lineOf(commit.body.blocks[ref.block], ref.item),
          spans: codeOf(commit.body.code, ref.block, ref.item),
          mentions: mentions.filter(it => it.block === ref.block && it.item === ref.item),
        })
        marks.set(section.key, list)
      }
    }
  }
  return marks
}
