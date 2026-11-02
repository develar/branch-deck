import type { CodeSpan, CommitBody, CommitBodyBlock, CommitBodyPart, LabelKind, LineCode, MessageChange, StoryRemoval } from "@/lib/bindings"

/*
 * The parts of a commit body as a reader shows them (spec/commit/commit-row.spec.md, "Message body"): Rust cuts the body
 * into its parts, says what each is and which fold (`CommitBody.parts`), and of a story where each is told
 * (`StoryCommit.parts`); this only places them, for every view of a body (the card of a commit, its preview, the opening
 * of a step of a review).
 */

/** A block of a body at its place in `CommitBody.blocks`: the mentions of a line name it by that place */
export interface PlacedBlock {
  readonly index: number
  readonly block: Exclude<CommitBodyBlock, { type: "label" | "heading" }>
  /** The reviewed body lacked it (`MessageChange`): marked as added since the review */
  readonly added?: true
}

/** Blocks of the reviewed body that the body lacks now, where a part shows them (`StoryRemoval`) */
export interface RemovedBlocks {
  /** Unique in its part */
  readonly key: string
  /** The block of the part they stand right before; null at its end */
  readonly before: number | null
  readonly blocks: readonly CommitBodyBlock[]
  /** The code of their lines, `block` by their place in `blocks` */
  readonly code: readonly LineCode[]
}

/**
 * A part of a body: a labelled section (a label or a heading and its blocks), else a run of blocks of no label; a run
 * the fold cuts is two parts
 */
export interface BodyPart {
  /** Its body and its first block: unique in a document */
  readonly key: string
  /** The place of its first block (its label or heading, else its first block) in `CommitBody.blocks` */
  readonly first: number
  /** Its label or heading at its place, null for blocks of no label */
  readonly head: { readonly index: number; readonly block: Extract<CommitBodyBlock, { type: "label" | "heading" }> } | null
  /** The kind of its label: a Look closely is a callout */
  readonly tone: LabelKind
  readonly blocks: readonly PlacedBlock[]
  /** Its label or heading is new since the review: the whole section is */
  readonly added?: true
  /** The blocks of the reviewed body it shows as removed since the review, each where they stood */
  readonly removed?: readonly RemovedBlocks[]
}

/** A part Rust folds: its own disclosure, named in the outline of the folded parts */
export interface FoldedPart {
  readonly part: BodyPart
  /** "The cause", "Tests"; "The rest" for blocks of no label */
  readonly name: string
}

/** A body as a view shows it: its lead, its parts in place, then the folded ones */
export interface BodyParts {
  /** The summary the body opens with; null when it opens with a label or is told elsewhere */
  readonly lead: PlacedBlock | null
  /** The blocks of the reviewed body the part of the lead shows as removed since the review */
  readonly leadRemoved?: readonly RemovedBlocks[]
  /** The parts before the fold and every Look closely, in the order of the body */
  readonly shown: readonly BodyPart[]
  /** The parts after the fold and every verification section, in the order of the body */
  readonly folded: readonly FoldedPart[]
  /** The spans of code of the lines of the body, as Rust found them (`CommitBody.code`) */
  readonly code: readonly LineCode[]
}

/** The spans of code of the line `item` of the block at `block` (`CommitBody.code`) */
export function codeOf(code: readonly LineCode[], block: number, item: number): readonly CodeSpan[] {
  return code.find(line => line.block === block && line.item === item)?.spans ?? NO_SPANS
}

const NO_SPANS: readonly CodeSpan[] = []

/** The name of a folded part of no label, in the line "More: The cause, The rest" */
export const MORE = "The rest"

/**
 * A part of a body as Rust cut and placed it: a part of its grammar (`CommitBody.parts`), or of a story a part told at
 * one place (`StoryPart`), whose label may be the title of its step (`titled`), which its part shows no more
 */
export type PlacedPart = Pick<CommitBodyPart, "start" | "end" | "kind" | "tone" | "folded"> & {
  readonly titled?: boolean
  /** Of a story whose message changed since the review: the removed blocks the part shows (`StoryPart.removals`) */
  readonly removals?: readonly StoryRemoval[]
}

/**
 * The parts of `body` (`BodyParts`), each keyed by `key` and its first block: of `parts` (all of the body by default),
 * the lead, then each part, the folded ones named; `fold` false shows every part in place. With `change`, how the
 * message changed since the review: its added blocks marked, the removed ones where Rust placed them (`removals`)
 */
export function presentBodyParts(key: string, body: CommitBody, parts: readonly PlacedPart[] = body.parts, fold = true, change: MessageChange | null = null): BodyParts {
  const { blocks } = body
  const added = new Set(change?.hunks.flatMap(hunk => hunk.added.blocks.map((_, at) => hunk.start + at)) ?? [])
  const removedOf = (removals: readonly StoryRemoval[] | undefined): readonly RemovedBlocks[] | undefined => {
    const runs = (removals ?? []).flatMap(({ hunk, before }) => {
      const removed = change?.hunks[hunk]?.removed
      return removed === undefined || removed.blocks.length === 0 ? [] : [{ key: `${key}:removed:${hunk}`, before, blocks: removed.blocks, code: removed.code }]
    })
    return runs.length === 0 ? undefined : runs
  }
  let lead: PlacedBlock | null = null
  let leadRemoved: readonly RemovedBlocks[] | undefined
  const view: { shown: BodyPart[]; folded: FoldedPart[] } = { shown: [], folded: [] }
  for (const { start, end, kind, tone, folded, titled = false, removals } of parts) {
    const first = blocks[start]
    const headBlock = kind === "section" && first !== undefined && (first.type === "label" || first.type === "heading") ? first : null
    const placed: PlacedBlock[] = []
    for (let index = headBlock === null ? start : start + 1; index < end; index++) {
      const block = blocks[index]!
      if (block.type !== "label" && block.type !== "heading") {
        placed.push(added.has(index) ? { index, block, added: true } : { index, block })
      }
    }
    const removed = removedOf(removals)
    if (kind === "lead") {
      lead = placed[0] ?? null
      leadRemoved = removed
      continue
    }
    const head = headBlock !== null && !titled ? { index: start, block: headBlock } : null
    if (placed.length === 0 && head === null && removed === undefined) {
      continue
    }
    const firstIndex = head?.index ?? placed[0]?.index ?? start
    const part: BodyPart = {
      key: `${key}:${firstIndex}`,
      first: firstIndex,
      head,
      tone,
      blocks: placed,
      ...(headBlock !== null && added.has(start) ? { added: true as const } : {}),
      ...(removed === undefined ? {} : { removed }),
    }
    if (folded && fold) {
      view.folded.push({ part, name: head === null ? MORE : head.block.text })
    } else {
      view.shown.push(part)
    }
  }
  return { lead, ...(leadRemoved === undefined ? {} : { leadRemoved }), ...view, code: body.code }
}
