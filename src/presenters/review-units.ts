import type { ApiChange, FileFacts, GeneratedBy, JoinedUnit, ReviewTitle, ReviewUnit, StoryRef, UnitBlock } from "@/lib/bindings"
import { plainTitle } from "@/lib/commit"
import { headingText } from "@/lib/diff/rows"
import type { Heading, HeadingPart, SectionUnits } from "@/lib/diff/rows"
import { joinNames } from "@/lib/text"
import { OTHER_CHANGES } from "@/presenters/review-concerns"
import { FOLDED_WORDS, FORM_WORDS, formatGenerated, MECHANICAL_WORDS } from "@/presenters/review-plan"
import { formatStepNumber, formatStepPosition } from "@/presenters/review-steps"
import type { Chapter } from "@/presenters/review-steps"

/*
 * The story of a review (openspec/specs/branch/review-units/spec.md, "In the Review Window"): the sections of the diffs in the
 * order Rust gave the units, each run of units of one file one section with the heading of each change, each group (a
 * kind of mechanical change, a repeated edit, the files deleted from one directory) a section per file of it. With the
 * concerns of the plan (openspec/specs/branch/review-concerns/spec.md), the parts of each concern together, in their order, under
 * its heading. Rust decides what each unit is, the concerns and the order; this words them and lays them out over the
 * files the review shows.
 */

/** A file the review shows, in the order of its tree, with the blocks of its diff once read (null before) */
export interface NarrativeFile {
  readonly path: string
  readonly blocks: readonly { readonly oldStart: number; readonly oldEnd: number; readonly newStart: number; readonly newEnd: number }[] | null
}

/** A section of the diffs: a file, or the blocks of it that a run of units or a group holds */
export interface NarrativeSection {
  /** Tells it from another section of the same file */
  readonly key: string
  readonly path: string
  /** The headings of its changes and the blocks other sections show; null for a file shown whole */
  readonly units: SectionUnits | null
  /** `ReviewUnit.id` of each unit it shows */
  readonly unitIds: ReadonlySet<string>
  /** The lines of the told bodies that name a stop it shows (`StepStop.marks`): its header marks them */
  readonly story: readonly StoryRef[]
  /** The section that shows each block of `units.elsewhere` */
  readonly targets: ReadonlyMap<number, string>
  /** The group it is of, on its first file: "Same edit in 3 files", "Imports only (2)" */
  readonly heading: string | null
  /**
   * The name of the group it is of, on each of its files, without the count of its heading: "Imports only", "Same
   * edit in 3 files"; the place a row and a fold of another section name it by, the bar above the diffs a section of no
   * concern by. Null for a section of no group
   */
  readonly groupName: string | null
  /** The opening of the group of a large step it starts (`ChapterPart.groupOpening`), by its key; null otherwise */
  readonly group: string | null
  /** `Concern.id` of the concern it is of; null without concerns, for the mechanical groups and a use left outside */
  readonly concern: string | null
  /** The name of its concern, on the first section of the concern: "proxy", "grep → accio", "Other changes" */
  readonly concernHeading: string | null
  /** Why it starts collapsed: a mechanical change, a file of a repeated edit after the first */
  readonly folded: string | null
  /** A use of the change before it that the change left outside: its file at the head around its line (from 1) */
  readonly ghost: { readonly line: number; readonly name: string; readonly heading: HeadingPart } | null
  /** Of a file the story shows in two sections or more, which one this is and what it changes; null otherwise */
  readonly part: SectionPart | null
}

/**
 * A part of a file the story shows in several sections: "part 2 of 3", its place among the sections of the file in the
 * order of the story, and the lines its blocks add and remove as Rust counts them (`UnitBlock.additions`)
 */
export interface SectionPart {
  /** From 0 */
  readonly index: number
  readonly count: number
  readonly additions: number
  readonly deletions: number
}

/** "part 2 of 3" */
export function formatPart(part: Pick<SectionPart, "index" | "count">): string {
  return `part ${part.index + 1} of ${part.count}`
}

/** The uses left outside that the story shows of a change at most, as the card lists them */
export const GHOSTS_PER_UNIT = 5

function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1)
}

/**
 * The change of `unit` as the heading of its block words it: its name in the code font and what changed ("open" and
 * "parameters changed", "close" and "body changed", "Fresh" and "new"); "Changes outside a declaration" of no name
 */
export function formatUnitHeading(unit: ReviewUnit): HeadingPart {
  const name = unit.name ?? ""
  const named = (words: string): HeadingPart => ({ name, words })
  switch (unit.kind.kind) {
    case "shape":
      return named(`${joinNames(unit.kind.changes.map(change => FORM_WORDS[change]))} changed`)
    case "behaviour":
      return named("body changed")
    case "added":
      return named("new")
    case "removed":
      return named("removed")
    case "newFile":
      return named("new file")
    case "deletedFile":
      return named("deleted file")
    case "deletedFiles":
      return { name: null, words: `Deleted from ${unit.kind.directory}/` }
    case "mechanical":
      return { name: null, words: MECHANICAL_WORDS[unit.kind.mechanical] }
    case "repeatedEdit":
      return { name: null, words: "Repeated edit" }
    case "other":
      return unit.kind.reason === "outside" ? { name: null, words: "Changes outside a declaration" } : { name, words: "" }
  }
}

/** "Same edit in 3 files", "Imports only", "Deleted from src/": the name of a group, without a count of its files */
function groupName(unit: ReviewUnit): string {
  if (unit.kind.kind === "repeatedEdit") {
    return `Same edit in ${unit.slices.length} files`
  }
  return unit.kind.kind === "mechanical" ? MECHANICAL_WORDS[unit.kind.mechanical] : headingText([formatUnitHeading(unit)])
}

/** "Same edit in 3 files", "Imports only (2)": the heading of a group on its first file, its name and the count of its files */
function groupHeading(unit: ReviewUnit): string {
  const name = groupName(unit)
  return unit.kind.kind === "mechanical" || unit.kind.kind === "deletedFiles" ? `${name} (${unit.slices.length})` : name
}

function isGroup(unit: ReviewUnit): boolean {
  return unit.kind.kind === "mechanical" || unit.kind.kind === "repeatedEdit" || unit.kind.kind === "deletedFiles"
}

/**
 * Why the section of the `index`th shown file of a group starts collapsed: the kind of a mechanical change (of a
 * generated file why, `generated`), a deletion from a directory, a file of a repeated edit after the first
 */
function groupFolded(unit: ReviewUnit, shown: readonly { readonly path: string }[], index: number, generated: GeneratedBy | undefined): string | null {
  switch (unit.kind.kind) {
    case "mechanical":
      return generated === undefined ? FOLDED_WORDS[unit.kind.mechanical] : formatGenerated(generated)
    case "deletedFiles":
      return "Deleted"
    default:
      return index === 0 ? null : `Same edit as ${fileName(shown[0]!.path)}`
  }
}

/** The blocks of a slice match those of the file's diff as the window reads it: the same lines on each side */
function sameBlocks(blocks: readonly UnitBlock[], file: NarrativeFile): boolean {
  return (
    file.blocks === null ||
    blocks.every(block => {
      const shown = file.blocks![block.index]
      return shown !== undefined && shown.oldStart === block.oldStart && shown.oldEnd === block.oldEnd && shown.newStart === block.newStart && shown.newEnd === block.newEnd
    })
  )
}

/** A section being laid out: its file, the units it holds (the blocks of each), its group */
interface Draft {
  key: string
  path: string
  /** The blocks it shows, null for the whole file */
  blocks: Set<number> | null
  marks: Map<number, Heading>
  /** The units it shows */
  unitIds?: Set<string>
  /** The lines of the told bodies that name what it shows */
  story?: StoryRef[]
  heading: string | null
  /** The name of the group it is of (`NarrativeSection.groupName`) */
  groupName: string | null
  /** The opening of the group it starts (`NarrativeSection.group`) */
  group?: string | null
  folded: string | null
  /** The uses left outside of its changes, shown after it */
  ghosts: NarrativeSection[]
  concern: string | null
  /** The name of its concern, for the row of a block it shows in another section */
  concernName: string | null
  concernHeading: string | null
}

/** A unit of the story with the slices, and of a slice the blocks, that one chapter holds of it */
interface Step {
  readonly unit: ReviewUnit
  readonly concern: Chapter | null
  /** The units without a block of their own that its blocks show (`StepStop.joined`), by the path of the slice */
  readonly joined: ReadonlyMap<string, readonly JoinedUnit[]>
  /** The lines of the told bodies that name its stops (`StepStop.marks`), by the path of the slice */
  readonly marks: ReadonlyMap<string, readonly StoryRef[]>
  /** It starts a group of its step, or the tests of one: the words of the divider above its section (`ChapterPart.groupHeading`) */
  readonly groupHeading?: string | null
  /** It starts a group that opens: the key of its opening (`ChapterPart.groupOpening`) */
  readonly groupOpening?: string | null
}

const NO_JOINED: ReadonlyMap<string, readonly JoinedUnit[]> = new Map()
const NO_JOINED_UNITS: readonly JoinedUnit[] = []
const NO_MARKS: ReadonlyMap<string, readonly StoryRef[]> = new Map()
const NO_STORY: readonly StoryRef[] = []

/**
 * The units in the order of the story: with a concern beside the rest, the parts of each concern in its order (a unit
 * of several concerns in each, with the slices and the blocks it holds there), then what no concern holds (the
 * mechanical groups) in the order of the units; else the units as Rust ordered them (`untoldSteps`)
 */
function storySteps(units: readonly ReviewUnit[], concerns: readonly Chapter[]): Step[] {
  if (!concerns.some(concern => concern.told)) {
    return untoldSteps(units, concerns)
  }
  const byId = new Map(units.map(unit => [unit.id, unit]))
  const held = new Set<string>()
  const steps: Step[] = []
  for (const concern of concerns) {
    for (const part of concern.parts) {
      const unit = byId.get(part.unit)
      const slice = unit?.slices.find(it => it.path === part.path)
      if (unit === undefined || slice === undefined) {
        continue
      }
      held.add(`${unit.id}\n${slice.path}`)
      // a unit without a block of its own is shown by the stop of the block of its line
      for (const joined of part.joined) {
        held.add(`${joined.unit}\n${slice.path}`)
      }
      // the blocks a concern holds of a slice, also of one the story shows whole (a file of no language): the others
      // are a row there that opens their concern
      const blocks = part.blocks
      const cut = blocks === null ? slice : { ...slice, whole: false, blocks: slice.blocks.filter(block => blocks.includes(block.index)) }
      const last = steps.at(-1)
      if (last !== undefined && last.concern === concern && last.unit.id === unit.id) {
        steps[steps.length - 1] = {
          concern,
          unit: { ...last.unit, slices: [...last.unit.slices, cut] },
          joined: new Map([...last.joined, [slice.path, part.joined]]),
          marks: new Map([...last.marks, [slice.path, part.marks]]),
        }
      } else {
        steps.push({
          concern,
          unit: { ...unit, slices: [cut] },
          joined: new Map([[slice.path, part.joined]]),
          marks: new Map([[slice.path, part.marks]]),
          groupHeading: part.groupHeading,
          groupOpening: part.groupOpening,
        })
      }
    }
  }
  for (const unit of units) {
    const left = unit.slices.filter(slice => !held.has(`${unit.id}\n${slice.path}`))
    if (left.length > 0) {
      steps.push({ concern: null, unit: { ...unit, slices: left }, joined: NO_JOINED, marks: NO_MARKS })
    }
  }
  return steps
}

/**
 * The units as Rust ordered them, of a story whose steps are not told: no concern, but what Rust placed in the stops of
 * its steps still holds, by the unit and the path of a stop (the units it shows beside its own, `StepStop.joined`, and
 * the lines of the told bodies that name it, `StepStop.marks`); a unit Rust joined to a stop is no step of its own there
 */
function untoldSteps(units: readonly ReviewUnit[], concerns: readonly Chapter[]): Step[] {
  const joinedOf = new Map<string, JoinedUnit[]>()
  const marksOf = new Map<string, StoryRef[]>()
  const held = new Set<string>()
  for (const part of concerns.flatMap(concern => concern.parts)) {
    const key = `${part.unit}\n${part.path}`
    joinedOf.set(key, [...(joinedOf.get(key) ?? []), ...part.joined])
    marksOf.set(key, [...(marksOf.get(key) ?? []), ...part.marks])
    for (const joined of part.joined) {
      held.add(`${joined.unit}\n${part.path}`)
    }
  }
  return units.flatMap(unit => {
    const slices = unit.slices.filter(slice => !held.has(`${unit.id}\n${slice.path}`))
    if (slices.length === 0) {
      return []
    }
    return [
      {
        unit: slices.length === unit.slices.length ? unit : { ...unit, slices },
        concern: null,
        joined: new Map(slices.map(slice => [slice.path, joinedOf.get(`${unit.id}\n${slice.path}`) ?? NO_JOINED_UNITS])),
        marks: new Map(slices.map(slice => [slice.path, marksOf.get(`${unit.id}\n${slice.path}`) ?? NO_STORY])),
      },
    ]
  })
}

/**
 * The sections of the diffs of `files` (the files the review shows, in tree order) in the order of `units` (Rust's),
 * with `concerns` beside the rest in the order of the concerns (`storySteps`): the first section of each under the name
 * of its concern, a generated file of a concern collapsed at its end, the mechanical groups after them.
 * A run of units of one file is one section, each change with its heading before its first block; a unit with no block
 * of its own joins the heading of the block of its line, which Rust names (`StepStop.joined`), marked with its step where
 * it is another's ("hooks — removed in step 1"). A group is a section per file: a mechanical one
 * collapsed, saying its kind; of a repeated edit the first file open under "Same edit in N files", the others collapsed.
 * A block of a file that another section shows is one row there, which opens that section. A file whose blocks do not
 * match the units (the window reads another whitespace policy), and a file no unit names, is one section of the whole
 * file: in the place of its first unit, else after the story. After the section of a change of a form or a removal, its
 * uses the change left outside (`api`, by the key and the file of the unit), each its file around the use.
 */
export function presentNarrative(
  units: readonly ReviewUnit[],
  files: readonly NarrativeFile[],
  api: readonly ApiChange[],
  facts: readonly FileFacts[],
  concerns: readonly Chapter[],
): NarrativeSection[] {
  const byPath = new Map(files.map(file => [file.path, file]))
  const unitOf = new Map(units.map(unit => [unit.id, unit]))
  const generatedOf = new Map(facts.flatMap(file => (file.generated === null ? [] : [[file.path, file.generated] as const])))
  const steps = storySteps(units, concerns)
  // a file whose blocks the units cannot cut shows whole
  const whole = new Set<string>()
  for (const slice of steps.flatMap(step => step.unit.slices)) {
    const file = byPath.get(slice.path)
    if (file !== undefined && !slice.whole && !sameBlocks(slice.blocks, file)) {
      whole.add(slice.path)
    }
  }
  const drafts: Draft[] = []
  const placed = new Set<string>()
  const counts = new Map<string, number>()
  const nextKey = (prefix: string, path: string) => {
    const base = `${prefix}:${path}`
    const count = counts.get(base) ?? 0
    counts.set(base, count + 1)
    return count === 0 ? base : `${base}#${count}`
  }
  // the concern of the step laid out, and its name until its first section takes it
  let concern: Chapter | null = null
  let concernHeading: string | null = null
  const own = (draft: Draft, unit: string) => {
    draft.unitIds ??= new Set()
    draft.unitIds.add(unit)
  }
  // the lines of the told bodies that name the stop of `step` in `path`, which the section of it shows
  const tell = (draft: Draft | undefined, step: Step, path: string) => {
    const refs = step.marks.get(path) ?? []
    if (draft !== undefined && refs.length > 0) {
      draft.story ??= []
      draft.story.push(...refs)
    }
  }
  const push = (draft: Omit<Draft, "concern" | "concernName" | "concernHeading">) => {
    const full = { ...draft, concern: concern?.id ?? null, concernName: concern?.reference ?? null, concernHeading }
    concernHeading = null
    drafts.push(full)
    return full
  }
  // the section of each file shown whole, which the units of the file are of
  const wholeDrafts = new Map<string, Draft>()
  const placeWhole = (path: string, unit?: string, heading: string | null = null, group: string | null = null) => {
    if (!placed.has(path)) {
      placed.add(path)
      wholeDrafts.set(path, push({ key: path, path, blocks: null, marks: new Map(), heading, groupName: null, group, folded: null, ghosts: [] }))
    }
    const draft = wholeDrafts.get(path)
    if (draft !== undefined && unit !== undefined) {
      own(draft, unit)
    }
    return draft
  }
  let last = null as { draft: Draft; unit: string } | null
  for (const step of steps) {
    const unit = step.unit
    // a group of a large step starts its own sections, its divider above the first
    const startsGroup = step.groupHeading ?? null
    if (startsGroup !== null) {
      last = null
    }
    if (step.concern !== concern) {
      // a concern starts its own sections, under its name
      last = null
      concern = step.concern
      concernHeading = concern?.heading ?? null
    }
    if (isGroup(unit)) {
      last = null
      const shown = unit.slices.filter(slice => byPath.has(slice.path))
      shown.forEach((slice, index) => {
        if (whole.has(slice.path)) {
          tell(placeWhole(slice.path, unit.id), step, slice.path)
          return
        }
        placed.add(slice.path)
        const first = index === 0
        const folded = groupFolded(unit, shown, index, generatedOf.get(slice.path))
        const draft = push({
          key: nextKey(`group:${unit.id}`, slice.path),
          path: slice.path,
          blocks: slice.whole ? null : new Set(slice.blocks.map(block => block.index)),
          marks: new Map(),
          // a generated file at the end of the concern of its source says why it is folded, no group
          heading: first && startsGroup !== null ? startsGroup : first && (concern === null || unit.kind.kind !== "mechanical") ? groupHeading(unit) : null,
          groupName: groupName(unit),
          group: first ? (step.groupOpening ?? null) : null,
          folded,
          ghosts: [],
        })
        own(draft, unit.id)
        tell(draft, step, slice.path)
      })
      continue
    }
    const slice = unit.slices[0]
    if (slice === undefined || !byPath.has(slice.path)) {
      continue
    }
    const joined = step.joined.get(slice.path) ?? []
    if (whole.has(slice.path) || slice.whole) {
      last = null
      tell(placeWhole(slice.path, unit.id, startsGroup, step.groupOpening ?? null), step, slice.path)
      for (const it of joined) {
        placeWhole(slice.path, it.unit)
      }
      continue
    }
    placed.add(slice.path)
    // a file of no language the review parses has no declaration to name: its header names it already
    const heading = unit.kind.kind === "other" && unit.kind.reason === "noLanguage" ? null : formatUnitHeading(unit)
    let draft: Draft | undefined = last?.draft.path === slice.path ? last.draft : undefined
    if (draft === undefined) {
      draft = push({
        key: nextKey("file", slice.path),
        path: slice.path,
        blocks: new Set(),
        marks: new Map(),
        heading: startsGroup,
        groupName: null,
        group: step.groupOpening ?? null,
        folded: null,
        ghosts: [],
      })
    }
    tell(draft, step, slice.path)
    const first = slice.blocks[0]
    if (first === undefined) {
      // a declaration without a block that Rust left a stop of its own, as no stop but a group's shows its line (Rust
      // joins the others to the stop of their block): no block to name it at, so its words join the heading before it
      const before = [...draft.marks.keys()].at(-1)
      if (before !== undefined && heading !== null) {
        draft.marks.set(before, [...draft.marks.get(before)!, heading])
        own(draft, unit.id)
      }
    } else {
      own(draft, unit.id)
      if (heading !== null) {
        draft.marks.set(first.index, [heading])
      }
      for (const block of slice.blocks) {
        draft.blocks!.add(block.index)
      }
      // the declarations without a block of their own on a line of its blocks: after the heading of their block, of
      // another step marked so
      for (const it of joined) {
        const other = unitOf.get(it.unit)
        if (other === undefined) {
          continue
        }
        own(draft, other.id)
        // another told step by its number, what no evidence joined by its name; in a story of no told step, none
        const of = concerns[it.step]
        const part = formatUnitHeading(other)
        const words: HeadingPart =
          of === undefined || of === step.concern || step.concern === null
            ? part
            : { ...part, words: `${part.words} in ${of.told ? formatStepNumber(of.number).toLowerCase() : of.reference}`.trimStart() }
        draft.marks.set(it.block, [...(draft.marks.get(it.block) ?? []), words])
      }
    }
    last = { draft, unit: unit.id }
    draft.ghosts.push(...ghostsOf(unit, slice.path, api))
  }
  concern = null
  concernHeading = null
  for (const file of files) {
    placeWhole(file.path)
  }

  const parts = partsOf(drafts, units)
  // the section of each block, for the rows that stand for it elsewhere
  const owners = new Map<string, Map<number, Draft>>()
  for (const draft of drafts) {
    for (const block of draft.blocks ?? []) {
      let ofPath = owners.get(draft.path)
      if (ofPath === undefined) {
        ofPath = new Map()
        owners.set(draft.path, ofPath)
      }
      ofPath.set(block, draft)
    }
  }
  return drafts.flatMap(draft => [sectionOf(draft, owners, parts), ...draft.ghosts])
}

/**
 * What the bar above the diffs names (openspec/specs/branch/review-tour/spec.md, "The bar of the step at the top is
 * one line", "The bar names what is no step alone"): the chapter of what is at the top (the step whose opening is
 * there, else the section; the first chapter before the diffs report one), a use left outside of the chapter of the
 * section before it. A step the story tells by its place and its title ("Step 2 / 4"), a chapter it tells as no step
 * (Other changes) by its title alone; a section of no chapter, after the story, by its group alone ("Imports only"),
 * of no group as the tree lists it, under Other changes
 */
export interface StepBarView {
  /** "Step 2 / 4"; null for what the story tells as no step */
  readonly position: string | null
  readonly title: ReviewTitle
  /** `title` as one line: its hover */
  readonly label: string
  readonly code: boolean
  /** The steps the story tells: "Overview · 4 steps" */
  readonly steps: number
  /** The chapter, whose opening a click shows; null for a section of no chapter */
  readonly chapter: Chapter | null
  /** Of a section of no chapter, the first section of its group, which a click shows */
  readonly section: { readonly key: string; readonly path: string } | null
}

/** The bar above the diffs at `top` (`StepBarView`); null for a story that tells no step, which has no bar */
export function presentStepBar(
  chapters: readonly Chapter[],
  narrative: readonly NarrativeSection[],
  top: { readonly opening: string | null; readonly section: string | null },
): StepBarView | null {
  const steps = chapters.filter(chapter => chapter.told).length
  if (steps === 0) {
    return null
  }
  const at = top.opening === null ? narrative.findIndex(section => section.key === top.section) : -1
  const id = top.opening ?? (at < 0 ? narrative.find(section => section.concern !== null)?.concern : concernAt(narrative, at))
  const ofChapter = (chapter: Chapter): StepBarView => ({
    position: chapter.told ? formatStepPosition(chapter.number, steps) : null,
    title: chapter.title,
    label: chapter.label,
    code: chapter.code,
    steps,
    chapter,
    section: null,
  })
  if (id !== null && id !== undefined) {
    const chapter = chapters.find(it => it.id === id)
    return chapter === undefined ? null : ofChapter(chapter)
  }
  const section = narrative[at]
  if (section === undefined) {
    return null
  }
  const rest = chapters.find(chapter => chapter.rest)
  if (section.groupName === null && rest !== undefined) {
    return ofChapter(rest)
  }
  // the heading of a group stands on its first file
  const first = narrative.find(it => it.concern === null && it.groupName === section.groupName && [...it.unitIds].some(unit => section.unitIds.has(unit))) ?? section
  const name = section.groupName ?? OTHER_CHANGES
  return { position: null, title: plainTitle(name), label: name, code: false, steps, chapter: null, section: { key: first.key, path: first.path } }
}

/** The concern of the section at `index`, of a use left outside the one of the section before it */
function concernAt(narrative: readonly NarrativeSection[], index: number): string | null {
  for (let at = index; at >= 0; at--) {
    const section = narrative[at]!
    if (section.ghost === null) {
      return section.concern
    }
  }
  return null
}

/**
 * The part each section of a file is (`SectionPart`), of a file the story shows in two sections or more: their place in
 * the order of the story, the lines of the blocks each shows (all of the file for a section of the whole file)
 */
function partsOf(drafts: readonly Draft[], units: readonly ReviewUnit[]): Map<Draft, SectionPart> {
  const byPath = new Map<string, Draft[]>()
  for (const draft of drafts) {
    byPath.set(draft.path, [...(byPath.get(draft.path) ?? []), draft])
  }
  // the lines of each block of each file, from the slices that hold it
  const linesOf = new Map<string, Map<number, UnitBlock>>()
  for (const slice of units.flatMap(unit => unit.slices)) {
    let ofPath = linesOf.get(slice.path)
    if (ofPath === undefined) {
      ofPath = new Map()
      linesOf.set(slice.path, ofPath)
    }
    for (const block of slice.blocks) {
      ofPath.set(block.index, block)
    }
  }
  const parts = new Map<Draft, SectionPart>()
  for (const [path, sections] of byPath) {
    if (sections.length < 2) {
      continue
    }
    const blocks = linesOf.get(path) ?? new Map<number, UnitBlock>()
    sections.forEach((draft, index) => {
      const shown = draft.blocks === null ? [...blocks.values()] : [...draft.blocks].flatMap(block => blocks.get(block) ?? [])
      parts.set(draft, {
        index,
        count: sections.length,
        additions: shown.reduce((sum, block) => sum + block.additions, 0),
        deletions: shown.reduce((sum, block) => sum + block.deletions, 0),
      })
    })
  }
  return parts
}

/** The first heading of a section as one line, else the name of its file */
function headingOrName(heading: Heading | undefined, path: string): string {
  return heading === undefined ? fileName(path) : headingText(heading)
}

/** The uses `unit` left outside, from its entry of `api` (a removal or a change of a form, by key and file) */
function ghostsOf(unit: ReviewUnit, path: string, api: readonly ApiChange[]): NarrativeSection[] {
  if (unit.kind.kind !== "shape" && unit.kind.kind !== "removed") {
    return []
  }
  const change = api.find(entry => entry.kind !== "added" && entry.key === unit.key && entry.path === path)
  const sites = change?.usages?.outsideSites.slice(0, GHOSTS_PER_UNIT) ?? []
  return sites.map(site => ({
    key: `ghost:${unit.id}:${site.path}:${site.line}`,
    path: site.path,
    units: null,
    unitIds: new Set(),
    story: NO_STORY,
    targets: new Map(),
    heading: null,
    groupName: null,
    group: null,
    concern: null,
    concernHeading: null,
    folded: null,
    ghost: { line: site.line, name: change!.name, heading: { name: change!.name, words: `left outside: ${site.path}:${site.line}` } },
    part: null,
  }))
}

/**
 * The section of `draft`: its headings, its part of its file (`parts`), and a row for each block another section shows
 * (`owners`, by path and block): of another concern that concern, of a group that folds it the group by its name
 * ("Shown in Imports only"), another part of the file by its number ("Shown in part 2"), else the heading of the owner
 */
function sectionOf(draft: Draft, owners: ReadonlyMap<string, ReadonlyMap<number, Draft>>, parts: ReadonlyMap<Draft, SectionPart>): NarrativeSection {
  {
    const { concern, concernHeading } = draft
    const part = parts.get(draft) ?? null
    if (draft.blocks === null) {
      return {
        key: draft.key,
        path: draft.path,
        units: null,
        unitIds: draft.unitIds ?? new Set(),
        story: draft.story ?? NO_STORY,
        targets: new Map(),
        heading: draft.heading,
        groupName: draft.groupName,
        group: draft.group ?? null,
        concern,
        concernHeading,
        folded: draft.folded,
        ghost: null,
        part,
      }
    }
    const elsewhere = new Map<number, string>()
    const targets = new Map<number, string>()
    for (const [block, owner] of owners.get(draft.path) ?? []) {
      if (owner !== draft) {
        // a block of another concern says which: the section of the same file there; of a group that folds it, the group
        // by its name ("Imports only"), never its count; of another part of the file in this concern, that part
        const ownerPart = parts.get(owner)
        const where =
          owner.concern !== draft.concern && owner.concernName !== null
            ? owner.concernName
            : owner.folded !== null && owner.groupName !== null
              ? owner.groupName
              : ownerPart !== undefined
                ? `part ${ownerPart.index + 1}`
                : (owner.heading ?? headingOrName(owner.marks.values().next().value, owner.path))
        elsewhere.set(block, `Shown in ${where}`)
        targets.set(block, owner.key)
      }
    }
    return {
      key: draft.key,
      path: draft.path,
      units: { marks: draft.marks, elsewhere },
      unitIds: draft.unitIds ?? new Set(),
      story: draft.story ?? NO_STORY,
      targets,
      heading: draft.heading,
      groupName: draft.groupName,
      group: draft.group ?? null,
      concern,
      concernHeading,
      folded: draft.folded,
      ghost: null,
      part,
    }
  }
}
