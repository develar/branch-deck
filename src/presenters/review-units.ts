import type { ApiChange, FileFacts, ReviewUnit, UnitBlock } from "@/lib/bindings"
import type { SectionUnits } from "@/lib/diff/rows"
import { FOLDED_WORDS, FORM_WORDS, formatGenerated, MECHANICAL_WORDS } from "@/presenters/review-plan"

/*
 * The story of a review (spec/branch/review-units.spec.md, "In the Review Window"): the sections of the diffs in the
 * order Rust gave the units, each run of units of one file one section with the heading of each change, each group (a
 * kind of mechanical change, a repeated edit) a section per file of it. Rust decides what each unit is and the order;
 * this words them and lays them out over the files the review shows.
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
  /** The section that shows each block of `units.elsewhere` */
  readonly targets: ReadonlyMap<number, string>
  /** The group it is of, on its first file: "Same edit in 3 files", "Imports only (2)" */
  readonly heading: string | null
  /** Why it starts collapsed: a mechanical change, a file of a repeated edit after the first */
  readonly folded: string | null
  /** A use of the change before it that the change left outside: its file at the head around its line (from 1) */
  readonly ghost: { readonly line: number; readonly name: string; readonly heading: string } | null
}

/** The uses left outside that the story shows of a change at most, as the card lists them */
export const GHOSTS_PER_UNIT = 5

function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1)
}

/** "open — form changed: parameters", "close — body changed", "Fresh — new", "Changes outside a declaration" */
export function formatUnitHeading(unit: ReviewUnit): string {
  const name = unit.name ?? ""
  switch (unit.kind.kind) {
    case "shape":
      return `${name} — form changed: ${unit.kind.changes.map(change => FORM_WORDS[change]).join(", ")}`
    case "behaviour":
      return `${name} — body changed`
    case "added":
      return `${name} — new`
    case "removed":
      return `${name} — removed`
    case "newFile":
      return `${name} — new file`
    case "mechanical":
      return MECHANICAL_WORDS[unit.kind.mechanical]
    case "repeatedEdit":
      return "Repeated edit"
    case "other":
      return unit.kind.reason === "outside" ? "Changes outside a declaration" : name
  }
}

/** "Same edit in 3 files", "Imports only (2)": the heading of a group on its first file */
function groupHeading(unit: ReviewUnit): string {
  const count = unit.slices.length
  if (unit.kind.kind === "repeatedEdit") {
    return `Same edit in ${count} files`
  }
  return unit.kind.kind === "mechanical" ? `${MECHANICAL_WORDS[unit.kind.mechanical]} (${count})` : formatUnitHeading(unit)
}

function isGroup(unit: ReviewUnit): boolean {
  return unit.kind.kind === "mechanical" || unit.kind.kind === "repeatedEdit"
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
  marks: Map<number, string>
  heading: string | null
  folded: string | null
  /** The uses left outside of its changes, shown after it */
  ghosts: NarrativeSection[]
}

/**
 * The sections of the diffs of `files` (the files the review shows, in tree order) in the order of `units` (Rust's).
 * A run of units of one file is one section, each change with its heading before its first block; a unit with no block
 * of its own joins the heading of the block that changed its lines. A group is a section per file: a mechanical one
 * collapsed, saying its kind; of a repeated edit the first file open under "Same edit in N files", the others collapsed.
 * A block of a file that another section shows is one row there, which opens that section. A file whose blocks do not
 * match the units (the window reads another whitespace policy), and a file no unit names, is one section of the whole
 * file: in the place of its first unit, else after the story. After the section of a change of a form or a removal, its
 * uses the change left outside (`api`, by the key and the file of the unit), each its file around the use.
 */
export function presentNarrative(units: readonly ReviewUnit[], files: readonly NarrativeFile[], api: readonly ApiChange[], facts: readonly FileFacts[]): NarrativeSection[] {
  const byPath = new Map(files.map(file => [file.path, file]))
  const generatedOf = new Map(facts.flatMap(file => (file.generated === null ? [] : [[file.path, file.generated] as const])))
  // a file whose blocks the units cannot cut shows whole
  const whole = new Set<string>()
  for (const slice of units.flatMap(unit => unit.slices)) {
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
  const placeWhole = (path: string) => {
    if (!placed.has(path)) {
      placed.add(path)
      drafts.push({ key: path, path, blocks: null, marks: new Map(), heading: null, folded: null, ghosts: [] })
    }
  }
  let last = null as { draft: Draft; unit: string } | null
  for (const unit of units) {
    if (isGroup(unit)) {
      last = null
      const shown = unit.slices.filter(slice => byPath.has(slice.path))
      shown.forEach((slice, index) => {
        if (whole.has(slice.path)) {
          placeWhole(slice.path)
          return
        }
        placed.add(slice.path)
        const first = index === 0
        const generated = generatedOf.get(slice.path)
        const folded =
          unit.kind.kind === "mechanical"
            ? generated === undefined
              ? FOLDED_WORDS[unit.kind.mechanical]
              : formatGenerated(generated)
            : first
              ? null
              : `Same edit as ${fileName(shown[0]!.path)}`
        drafts.push({
          key: nextKey(`group:${unit.id}`, slice.path),
          path: slice.path,
          blocks: slice.whole ? null : new Set(slice.blocks.map(block => block.index)),
          marks: new Map(),
          heading: first ? groupHeading(unit) : null,
          folded,
          ghosts: [],
        })
      })
      continue
    }
    const slice = unit.slices[0]
    if (slice === undefined || !byPath.has(slice.path)) {
      continue
    }
    if (whole.has(slice.path) || slice.whole) {
      last = null
      placeWhole(slice.path)
      continue
    }
    placed.add(slice.path)
    const heading = formatUnitHeading(unit)
    let draft: Draft | undefined = last?.draft.path === slice.path ? last.draft : undefined
    if (draft === undefined) {
      draft = { key: nextKey("file", slice.path), path: slice.path, blocks: new Set(), marks: new Map(), heading: null, folded: null, ghosts: [] }
      drafts.push(draft)
    }
    const first = slice.blocks[0]
    if (first === undefined) {
      // a declaration whose block another unit holds: its words join the heading before it
      const before = [...draft.marks.keys()].at(-1)
      if (before !== undefined) {
        draft.marks.set(before, `${draft.marks.get(before)} · ${heading}`)
      }
    } else {
      draft.marks.set(first.index, heading)
      for (const block of slice.blocks) {
        draft.blocks!.add(block.index)
      }
    }
    last = { draft, unit: unit.id }
    draft.ghosts.push(...ghostsOf(unit, slice.path, api))
  }
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
    targets: new Map(),
    heading: null,
    folded: null,
    ghost: { line: site.line, name: change!.name, heading: `${change!.name} — left outside: ${site.path}:${site.line}` },
  }))
}

/** The section of `draft`: its headings, and a row for each block another section shows (`owners`, by path and block) */
function sectionOf(draft: Draft, owners: ReadonlyMap<string, ReadonlyMap<number, Draft>>): NarrativeSection {
  {
    if (draft.blocks === null) {
      return { key: draft.key, path: draft.path, units: null, targets: new Map(), heading: draft.heading, folded: draft.folded, ghost: null }
    }
    const elsewhere = new Map<number, string>()
    const targets = new Map<number, string>()
    for (const [block, owner] of owners.get(draft.path) ?? []) {
      if (owner !== draft) {
        elsewhere.set(
          block,
          `Shown in ${owner.heading ?? (owner.folded !== null ? `${owner.folded} · ${fileName(owner.path)}` : (owner.marks.values().next().value ?? fileName(owner.path)))}`,
        )
        targets.set(block, owner.key)
      }
    }
    return { key: draft.key, path: draft.path, units: { marks: draft.marks, elsewhere }, targets, heading: draft.heading, folded: draft.folded, ghost: null }
  }
}
