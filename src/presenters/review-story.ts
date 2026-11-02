import type { CodeSpan, CommitBodyBlock, CommitRole, ContractKind, Mention, MentionTarget, ReviewPlan, ReviewStep, ReviewUnit, StoryCommit } from "@/lib/bindings"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import type { BodyPart, BodyParts, PlacedBlock } from "@/presenters/commit-body"
import { codeOf, presentBodyParts } from "@/presenters/commit-body"
import type { EvidencePart } from "@/presenters/review-concerns"
import { presentConcernEvidence } from "@/presenters/review-concerns"
import type { Breakage } from "@/presenters/review-plan"
import { firstUse, formatPairState, formatRewrittenChanges, formatSinceReview, presentBreakage } from "@/presenters/review-plan"
import { formatStepNumber, headOf, presentChapters } from "@/presenters/review-steps"
import type { NarrativeSection } from "@/presenters/review-units"

/*
 * The words of the review as one document (spec/branch/review-tour.spec.md, "In the Review Window"): the overview at
 * its top (`presentPrologue`), then the opening of each step above the card of its first section (`presentOpenings`).
 * Rust tells the steps, the story of each commit by the grammar of its body, its code names, where the fold of a body
 * is and, of the concerns of one commit, where each block is told; this only words and places them.
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
}

/**
 * The story of `commit` as the document shows it: of the blocks `told` keeps (all by default), its parts as Rust cut
 * and folded them (`presentBodyParts`), its code names worded; `titled` the label Rust made the title of its step
 */
export function presentStory(commit: StoryCommit, told: (index: number) => boolean = () => true, words: MentionWords = plainWords, titled: number | null = null): StoryView {
  return { commit: commit.id, mentions: wordMentions(commit, words), ...presentBodyParts(commit.id, commit.body, told, true, titled) }
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

/** A story of the steps of the concerns of one commit: the blocks told before the steps, and those of each step */
function storyOfOneCommit(plan: Pick<ReviewPlan, "steps">): StoryCommit | null {
  const { source, commits } = plan.steps
  return source.kind === "concerns" && source.reason === "oneCommit" && commits.length === 1 ? commits[0]! : null
}

/** A name of a fact of a step that shows its place: a new declaration, a new link between modules at its first use */
export interface FactPlace {
  readonly key: string
  /** "Cache", "Reader.kt → Cache.kt" */
  readonly label: string
  readonly place: ReviewPlace
}

/** "Tests of this change reach allowed", "Proven by restart.json": the facts of a step, and its warning */
export interface StepFacts {
  /** The tests that reach its code and that nothing of it lands, apart in one line; null for none */
  readonly line: string | null
  /** The public declarations it adds ("New API:"), each at its declaration */
  readonly newApi: readonly FactPlace[]
  /** The links it adds between two modules ("New link between modules:"), each at its first use */
  readonly moduleLinks: readonly FactPlace[]
  /**
   * Of a step none of whose code lands, the step of the later commit that took it out ("Nothing of it lands: step 5
   * took it out"): its words, the title of that commit in their hover, and the id of the step, which a click shows; null
   * otherwise, and for a step that lands nothing with no such commit known the line says "a later commit"
   */
  readonly takenOutBy: { readonly words: string; readonly title: string; readonly step: string | null } | null
  /**
   * Its units the section of another step shows (`ReviewStep.shownElsewhere`), by what it does to them and that step:
   * "Removes hooks, shown in step 2", "step 2" showing that step
   */
  readonly shownElsewhere: readonly { readonly key: string; readonly words: string; readonly step: string; readonly stepId: string }[]
  /**
   * accio answered, and no test reaches any of its code and no scenario proves it: "No test reaches paint", "No test
   * reaches paint within 3 calls" when the walk stopped with callers left
   */
  readonly unproven: string | null
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

/**
 * The facts of `step` (`StepFacts`): its code the tests of the change reach, the tests it did not touch that reach its
 * code, the scenarios that prove it, its code no test reaches beside proven code, the public declarations it adds and
 * the links it adds between modules, and that nothing of it lands
 */
export function presentStepFacts(step: ReviewStep, plan: Pick<ReviewPlan, "api" | "links" | "steps" | "units">): StepFacts {
  const { answered, reached, unreached, scenarios } = step.tests
  const ofChange = reached.filter(declaration => declaration.tests.some(test => test.unit !== null))
  const untouched = new Set(reached.flatMap(declaration => declaration.tests.filter(test => test.unit === null).map(test => `${test.path}:${test.line}`)))
  const coveredNames = reached.filter(declaration => declaration.tests.some(test => test.unit === null)).map(it => it.name)
  const within = Math.max(0, ...unreached.map(declaration => declaration.within ?? 0))
  const none = unreached.length === 0 ? null : `No test reaches ${names(unreached.map(it => it.name))}`
  const newApi = new Set(step.newApi)
  const added = plan.api
    .filter(change => change.kind === "added" && newApi.has(change.key))
    .map(change => ({
      key: `${change.key}:${change.path}:${change.line}`,
      label: change.name,
      place: { path: change.path, side: "new" as const, line: change.line, name: change.name },
    }))
  const links = step.moduleLinks.flatMap(index => {
    const link = plan.links[index]
    return link === undefined ? [] : [{ key: `${index}`, label: `${fileName(link.from)} → ${fileName(link.to)}`, place: firstUse(link) }]
  })
  const takenOutBy = takenOutOf(step, plan)
  const shownElsewhere = shownElsewhereOf(step, plan)
  const parts = [
    ofChange.length === 0 ? null : `Tests of this change reach ${names(ofChange.map(it => it.name))}`,
    untouched.size === 0 ? null : `${untouched.size === 1 ? "1 test it did not touch reaches" : `${untouched.size} tests it did not touch reach`} ${names(coveredNames)}`,
    scenarios.length === 0 ? null : `Proven by ${names(scenarios.map(fileName))}`,
    none !== null && (reached.length > 0 || scenarios.length > 0) ? none : null,
    step.stops.length === 0 && step.shownElsewhere.length === 0 && takenOutBy === null ? "Nothing of it lands: a later commit took it out" : null,
  ].filter(part => part !== null)
  return {
    line: parts.length === 0 ? null : parts.join(" · "),
    newApi: added,
    moduleLinks: links,
    takenOutBy,
    shownElsewhere,
    unproven: answered && none !== null && reached.length === 0 && scenarios.length === 0 ? (within > 0 ? `${none} within ${within} calls` : none) : null,
  }
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

/** The units of `step` that a section of another step shows, by what it does to them and that step, in their order */
function shownElsewhereOf(step: ReviewStep, plan: Pick<ReviewPlan, "steps" | "units">): StepFacts["shownElsewhere"] {
  const units = new Map(plan.units.map(unit => [unit.id, unit]))
  const groups: { key: string; verb: string; names: string[]; step: number }[] = []
  for (const shown of step.shownElsewhere) {
    const unit = units.get(shown.unit)
    const verb = verbOf(unit)
    const name = unit?.name ?? shown.unit
    const group = groups.find(it => it.verb === verb && it.step === shown.step)
    if (group === undefined) {
      groups.push({ key: `${verb}:${shown.step}`, verb, names: [name], step: shown.step })
    } else {
      group.names.push(name)
    }
  }
  return groups.flatMap(group => {
    const holder = plan.steps.steps[group.step]
    return holder === undefined ? [] : [{ key: group.key, words: `${group.verb} ${names(group.names)}, shown in`, step: `step ${group.step + 1}`, stepId: holder.id }]
  })
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
  readonly number: number
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

/** A file of What Changes: its kind and path once, then each changed block of it */
export interface ContractFile {
  readonly key: string
  readonly path: string
  /** "Spec rule", "Decision", "Agent guide", "Doc" */
  readonly kind: string
  readonly entries: readonly {
    readonly key: string
    /** The heading of its section of Markdown */
    readonly heading: string | null
    /** Its paragraphs, each with its spans of code as Rust found them */
    readonly paragraphs: readonly ProseParagraph[]
    readonly removed: readonly ProseParagraph[]
    /** "Proven by restart.json" */
    readonly provenBy: string | null
  }[]
}

/** A paragraph of prose with its spans of code as Rust found them (`sync_utils::code_spans`) */
export interface ProseParagraph {
  readonly text: string
  readonly code: readonly CodeSpan[]
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
  readonly contract: readonly ContractFile[]
  /** The contents: the steps of the story; none when the only step is what no evidence joined */
  readonly steps: readonly ContentsStep[]
  /** Why the tasks of the range are its steps, a muted line under the label of the steps: "One commit: the steps are its tasks" */
  readonly source: string | null
  /** Without a step in the contents, the facts of the range: the new API, the new links between modules; null with steps */
  readonly facts: StepFacts | null
  readonly breakage: Breakage
  /** "Since your review: 1 new · 1 rewritten" and the lines of each rewritten commit; null without a change since */
  readonly since: { readonly summary: string; readonly rewritten: readonly { readonly key: string; readonly text: string; readonly title: string }[] } | null
  /**
   * Why the uses are unknown ("no index of this repository"): folded under Possible Breakage when it lists an entry,
   * else one muted line of its own ("Uses unknown: no index of this repository")
   */
  readonly indexNote: { readonly text: string; readonly underBreakage: boolean } | null
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
  const one = storyOfOneCommit(plan)
  const steps = (told ? plan.steps.steps : []).map((step, index): ContentsStep => {
    const chapter = chapters[index]!
    const head = headOf(step)
    const story = step.concern !== null ? undefined : head?.role === "noStory" ? undefined : storyOf(plan, head?.id)
    const view = story === undefined ? null : presentStory(story, undefined, words)
    const toldHere = one === null || step.concern === null ? null : presentStory(one, block => one.toldIn[block] === index, words, step.toldAsTitle)
    return {
      id: step.id,
      number: chapter.number,
      title: chapter.label,
      code: chapter.code,
      lead: view?.lead == null ? null : { story: view, block: view.lead },
      facts: presentStepFacts(step, plan),
      // the author's question of a round too: its story is folded under the step's opening
      lookClosely: view !== null ? [view, ...followUpStories(step, plan, words)].flatMap(lookCloselyOf) : toldHere === null ? [] : lookCloselyOf(toldHere),
    }
  })
  const since = plan.interdiff === null ? null : formatSinceReview(plan.interdiff)
  const contract: { key: string; path: string; kind: string; entries: ContractFile["entries"][number][] }[] = []
  plan.brief.contract.forEach((entry, index) => {
    const entryView = {
      key: `${entry.unit}:${entry.block ?? "file"}:${index}`,
      heading: entry.heading,
      paragraphs:
        entry.newSpec !== null
          ? [{ text: entry.newSpec.description ?? `A new spec of ${entry.newSpec.rules} rules`, code: [] }]
          : entry.deleted
            ? [{ text: "Deleted", code: [] }]
            : entry.added.map((text, index) => ({ text, code: entry.addedCode[index] ?? [] })),
      removed: entry.removed.map((text, index) => ({ text, code: entry.removedCode[index] ?? [] })),
      provenBy: entry.provenBy.length === 0 ? null : `Proven by ${names(entry.provenBy.map(fileName))}`,
    }
    const last = contract.at(-1)
    if (last?.path === entry.path) {
      last.entries.push(entryView)
    } else {
      contract.push({ key: entryView.key, path: entry.path, kind: CONTRACT_WORDS[entry.kind], entries: [entryView] })
    }
  })
  const breakage = presentBreakage(plan.api, pending)
  return {
    issue: plan.brief.issue,
    title: plan.title,
    // without a step of a task the diffs have no opening: the whole story is told here
    untold: one === null ? null : { subject: one.title, story: presentStory(one, block => !told || (one.toldIn[block] ?? null) === null, words) },
    contract,
    steps,
    source:
      !told || plan.steps.source.kind === "commits"
        ? null
        : plan.steps.source.reason === "oneCommit"
          ? "One commit: the steps are its tasks"
          : "The commits tell no story: the steps are the tasks of the range",
    facts: told ? null : rangeFacts(plan),
    breakage,
    since:
      since === null || plan.interdiff === null
        ? null
        : {
            summary: `Since your review: ${since}`,
            rewritten: plan.interdiff.commits
              .filter(commit => commit.state === "rewritten" && commit.changes !== null)
              .map((commit, index) => ({
                key: `${commit.id ?? index}`,
                text: `${formatPairState(commit.state)} ${commit.subject}: ${formatRewrittenChanges(commit.changes!)}`,
                title: commit.subject,
              })),
          },
    // folded under Possible Breakage where the overview lists an entry of it, the uses it tells of
    indexNote: plan.indexNote === null ? null : { text: `Uses unknown: ${plan.indexNote}`, underBreakage: breakage.entries.length > 0 },
  }
}

/** The facts of every step of `plan` as one: of a range whose contents list no step; null without a fact */
function rangeFacts(plan: ReviewPlan): StepFacts | null {
  const all = plan.steps.steps.map(step => presentStepFacts(step, plan))
  const lines = all.flatMap(facts => (facts.line === null ? [] : [facts.line]))
  const facts = {
    line: lines.length === 0 ? null : lines.join(" · "),
    newApi: all.flatMap(it => it.newApi),
    moduleLinks: all.flatMap(it => it.moduleLinks),
    takenOutBy: null,
    shownElsewhere: [],
    unproven: null,
  }
  return facts.line === null && facts.newApi.length === 0 && facts.moduleLinks.length === 0 ? null : facts
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
  /** "Step 2" */
  readonly number: string
  /** Its full title, which wraps */
  readonly title: string
  /** Its title is an identifier: set as code */
  readonly code: boolean
  /** Of a step of a concern: what joins its parts ("Joined by the identifiers" `hide`, `show`), in the line of its facts */
  readonly evidence: readonly EvidencePart[]
  /** The story it tells: the body of the commit that starts it, of a concern of one commit the blocks told in it */
  readonly story: StoryView | null
  /** The commits after the one that starts it: those that follow it up, then those of no story whose lines are mostly in it */
  readonly commits: readonly OpeningCommit[]
  readonly facts: StepFacts
}

/** The opening of each step of `plan` by its id (`OpeningView`) */
export function presentOpenings(plan: Pick<ReviewPlan, "steps" | "concerns" | "api" | "links" | "units">): Map<string, OpeningView> {
  const chapters = presentChapters(plan)
  const concerns = new Map(plan.concerns.map(concern => [concern.id, concern]))
  const one = storyOfOneCommit(plan)
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
      const head = concern === undefined ? headOf(step) : undefined
      const starts = head?.role === "starts" ? head : undefined
      const story =
        concern !== undefined
          ? one === null
            ? null
            : presentStory(one, block => one.toldIn[block] === index, words, step.toldAsTitle)
          : starts === undefined
            ? null
            : commitOf(starts.id, starts.title, "starts").story
      const rest = concern === undefined ? step.commits.filter(commit => commit !== starts) : []
      return [
        step.id,
        {
          id: step.id,
          number: formatStepNumber(chapter.number),
          title: chapter.label,
          code: chapter.code,
          evidence: concern === undefined ? [] : presentConcernEvidence(concern.evidence),
          story: story === null || (story.lead === null && story.shown.length === 0 && story.folded.length === 0) ? null : story,
          commits: [...rest.filter(commit => commit.role === "followsUp"), ...rest.filter(commit => commit.role === "noStory")].map(commit =>
            commitOf(commit.id, commit.title, commit.role),
          ),
          facts: presentStepFacts(step, plan),
        },
      ]
    }),
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

/** The text of the line `item` of a block: a line of a paragraph, an item of a list, a heading or a label */
function lineOf(block: CommitBodyBlock | undefined, item: number): string {
  switch (block?.type) {
    case "paragraph":
      return block.lines[item] ?? ""
    case "list":
      return block.items[item] ?? ""
    case "heading":
    case "label":
      return block.text
    default:
      return ""
  }
}

/**
 * The marks of each section of the story by its key: a section a resolved mention of a told body names (the section
 * of its path in the step of its target; of a declaration, the section that holds its unit when one does),
 * one mark per block that names it. A body is told where the document shows it: the commit that starts a step in its
 * opening, the concerns of one commit where `toldIn` says, the rest in the overview; a follow-up is folded under its
 * step and marks nothing
 */
export function storyMarks(plan: Pick<ReviewPlan, "steps" | "concerns" | "units">, sections: readonly NarrativeSection[]): Map<string, StoryMark[]> {
  const chapters = presentChapters(plan)
  const words = mentionWords(plan)
  const steps = plan.steps.steps
  const told = chapters.some(chapter => chapter.told)
  const one = storyOfOneCommit(plan)
  // where each told commit is told: its step, or per block for the concerns of one commit
  const toldAt = new Map<string, (block: number) => number | null>()
  if (one !== null) {
    toldAt.set(one.id, block => (told ? (one.toldIn[block] ?? null) : null))
  } else {
    steps.forEach((step, index) => {
      const head = headOf(step)
      if (step.concern === null && head?.role === "starts") {
        toldAt.set(head.id, () => index)
      }
    })
  }
  const marks = new Map<string, StoryMark[]>()
  for (const commit of plan.steps.commits) {
    const at = toldAt.get(commit.id)
    if (at === undefined) {
      continue
    }
    // worded once per commit: a mark quotes the worded mentions of its line
    const worded = wordMentions(commit, words)
    for (const mention of commit.mentions) {
      const target = mention.target
      if (target === null) {
        continue
      }
      const stepId = target.step === null ? null : (steps[target.step]?.id ?? null)
      const candidates = sections.filter(section => section.ghost === null && section.path === target.path && (stepId === null || section.concern === stepId))
      const named = target.kind === "unit" ? candidates.filter(section => section.unitIds.has(target.unit)) : []
      const toldIndex = at(mention.block)
      const where = toldIndex === null ? null : (steps[toldIndex] ?? null)
      const chapter = toldIndex === null ? undefined : chapters[toldIndex]
      for (const section of named.length > 0 ? named : candidates) {
        const list = marks.get(section.key) ?? []
        if (!list.some(mark => mark.commit === commit.id && mark.block === mention.block)) {
          list.push({
            key: `${commit.id}:${mention.block}`,
            commit: commit.id,
            block: mention.block,
            step: where?.id ?? null,
            title: chapter === undefined ? "Overview" : `${formatStepNumber(chapter.number)} · ${chapter.label}`,
            place: chapter === undefined ? "Overview" : formatStepNumber(chapter.number),
            label: chapter?.label ?? null,
            code: chapter?.code ?? false,
            preview: lineOf(commit.body.blocks[mention.block], mention.item),
            spans: codeOf(commit.body.code, mention.block, mention.item),
            mentions: worded.filter(it => it.block === mention.block && it.item === mention.item),
          })
          marks.set(section.key, list)
        }
      }
    }
  }
  return marks
}
