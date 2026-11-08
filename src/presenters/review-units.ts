import type { ApiChange, FileFacts, GeneratedBy, JoinedUnit, ReviewUnit, StoryRef, UnitBlock } from "@/lib/bindings"
import { headingText } from "@/lib/diff/rows"
import type { Heading, HeadingPart, SectionUnits } from "@/lib/diff/rows"
import { joinNames } from "@/lib/text"
import { FOLDED_WORDS, FORM_WORDS, formatGenerated, MECHANICAL_WORDS } from "@/presenters/review-plan"
import { formatStepNumber } from "@/presenters/review-steps"
import type { Chapter } from "@/presenters/review-steps"

/*
 * The story of a review (spec/branch/review-units.spec.md, "In the Review Window"): the sections of the diffs in the
 * order Rust gave the units, each run of units of one file one section with the heading of each change, each group (a
 * kind of mechanical change, a repeated edit, the files deleted from one directory) a section per file of it. With the
 * concerns of the plan (spec/branch/review-concerns.spec.md), the parts of each concern together, in their order, under
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
  /** `Concern.id` of the concern it is of; null without concerns, for the mechanical groups and a use left outside */
  readonly concern: string | null
  /** The name of its concern, on the first section of the concern: "proxy", "grep → accio", "Other changes" */
  readonly concernHeading: string | null
  /** Why it starts collapsed: a mechanical change, a file of a repeated edit after the first */
  readonly folded: string | null
  /** A use of the change before it that the change left outside: its file at the head around its line (from 1) */
  readonly ghost: { readonly line: number; readonly name: string; readonly heading: HeadingPart } | null
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

/** "Same edit in 3 files", "Imports only (2)": the heading of a group on its first file */
function groupHeading(unit: ReviewUnit): string {
  const count = unit.slices.length
  if (unit.kind.kind === "repeatedEdit") {
    return `Same edit in ${count} files`
  }
  if (unit.kind.kind === "deletedFiles") {
    return `Deleted from ${unit.kind.directory}/ (${count})`
  }
  return unit.kind.kind === "mechanical" ? `${MECHANICAL_WORDS[unit.kind.mechanical]} (${count})` : headingText([formatUnitHeading(unit)])
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
  const placeWhole = (path: string, unit?: string, heading: string | null = null) => {
    if (!placed.has(path)) {
      placed.add(path)
      wholeDrafts.set(path, push({ key: path, path, blocks: null, marks: new Map(), heading, folded: null, ghosts: [] }))
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
      tell(placeWhole(slice.path, unit.id, startsGroup), step, slice.path)
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
      draft = push({ key: nextKey("file", slice.path), path: slice.path, blocks: new Set(), marks: new Map(), heading: startsGroup, folded: null, ghosts: [] })
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
  return drafts.flatMap(draft => [sectionOf(draft, owners), ...draft.ghosts])
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
    concern: null,
    concernHeading: null,
    folded: null,
    ghost: { line: site.line, name: change!.name, heading: { name: change!.name, words: `left outside: ${site.path}:${site.line}` } },
  }))
}

/** The section of `draft`: its headings, and a row for each block another section shows (`owners`, by path and block) */
function sectionOf(draft: Draft, owners: ReadonlyMap<string, ReadonlyMap<number, Draft>>): NarrativeSection {
  {
    const { concern, concernHeading } = draft
    if (draft.blocks === null) {
      return {
        key: draft.key,
        path: draft.path,
        units: null,
        unitIds: draft.unitIds ?? new Set(),
        story: draft.story ?? NO_STORY,
        targets: new Map(),
        heading: draft.heading,
        concern,
        concernHeading,
        folded: draft.folded,
        ghost: null,
      }
    }
    const elsewhere = new Map<number, string>()
    const targets = new Map<number, string>()
    for (const [block, owner] of owners.get(draft.path) ?? []) {
      if (owner !== draft) {
        // a block of another concern says which: the section of the same file there
        const where =
          owner.concern !== draft.concern && owner.concernName !== null
            ? owner.concernName
            : (owner.heading ?? (owner.folded !== null ? `${owner.folded} · ${fileName(owner.path)}` : headingOrName(owner.marks.values().next().value, owner.path)))
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
      concern,
      concernHeading,
      folded: draft.folded,
      ghost: null,
    }
  }
}
