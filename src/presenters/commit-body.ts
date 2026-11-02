import type { CodeSpan, CommitBody, CommitBodyBlock, LabelKind, LineCode } from "@/lib/bindings"

/*
 * The parts of a commit body as a reader shows them (spec/commit/commit-row.spec.md, "Message body"): Rust cuts the body
 * into its parts and says which fold (`CommitBody.parts`); this only places them, for every view of a body (the card of
 * a commit, its preview, the opening of a step of a review).
 */

/** A block of a body at its place in `CommitBody.blocks`: the mentions of a line name it by that place */
export interface PlacedBlock {
  readonly index: number
  readonly block: Exclude<CommitBodyBlock, { type: "label" | "heading" }>
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
 * The parts of `body` (`BodyParts`), each keyed by `key` and its first block: of the blocks `told` keeps (all by
 * default), the lead, then the parts as Rust cut them, the folded ones named; `fold` false shows every part in place.
 * `titled`: the label Rust made the title of the step that tells it (`ReviewStep.toldAsTitle`), which its part shows
 * no more: the opening names it already
 */
export function presentBodyParts(key: string, body: CommitBody, told: (index: number) => boolean = () => true, fold = true, titled: number | null = null): BodyParts {
  const { blocks, parts, lead } = body
  let leadBlock: PlacedBlock | null = null
  const view: { shown: BodyPart[]; folded: FoldedPart[] } = { shown: [], folded: [] }
  for (const { start, end, folded } of parts) {
    const first = blocks[start]
    const headBlock = first !== undefined && (first.type === "label" || first.type === "heading") ? first : null
    const headed = headBlock !== null
    const head = headBlock !== null && start !== titled ? { index: start, block: headBlock } : null
    // Rust tells a labelled section whole, at one place; of a run of blocks of no label, a block told apart from the
    // one before it starts a part
    const runs: PlacedBlock[][] = []
    for (let index = headed ? start + 1 : start; index < end; index++) {
      const block = blocks[index]!
      if (!told(index) || block.type === "label" || block.type === "heading") {
        continue
      }
      if (index === 0 && lead && block.type === "paragraph") {
        leadBlock = { index, block }
        continue
      }
      const run = runs.at(-1)
      if (run === undefined || run.at(-1)!.index !== index - 1) {
        runs.push([{ index, block }])
      } else {
        run.push({ index, block })
      }
    }
    if (headed && !told(start)) {
      continue
    }
    const tone = head?.block.type === "label" ? head.block.kind : "plain"
    // a labelled section is one part, its label the title of its step or not
    for (const placed of headed ? [runs.flat()] : runs) {
      if (placed.length === 0 && head === null) {
        continue
      }
      const firstIndex = head?.index ?? placed[0]!.index
      const part: BodyPart = { key: `${key}:${firstIndex}`, first: firstIndex, head, tone, blocks: placed }
      if (folded && fold) {
        view.folded.push({ part, name: head === null ? MORE : head.block.text })
      } else {
        view.shown.push(part)
      }
    }
  }
  return { lead: leadBlock, ...view, code: body.code }
}
