import type { CommitBodyBlock } from "@/lib/bindings"

/** Blocks under a lead-in label ("Why:", "What changed:"); the leading blocks and the trailers have no label. */
export interface CommitBodySection {
  label: string | null
  blocks: CommitBodyBlock[]
}

// a whole paragraph that introduces the next blocks: "What changed:", "Gates on the final tree, each exit 0:"
const LEAD_IN_PARAGRAPH = /^([A-Z][^:]{0,38}):$/
// a short run-in label at the start of a paragraph: "Why: …", "Left out: …"
const RUN_IN_LABEL = /^([A-Z][a-z]*(?: [a-z]+){0,2}): (.+)$/
const RUN_IN_LABEL_MAX_LENGTH = 20

/**
 * Groups the body blocks Rust parsed (`commit_body_blocks`) into sections that start at a lead-in label:
 * - a paragraph that is only `Label:` (up to 40 characters) labels the blocks that follow it;
 * - a paragraph starting with a 1–3 word `Label: ` (up to 20 characters) keeps the rest of its text as the first block.
 *
 * Unlabelled blocks join the current section; trailers always form the last, unlabelled section.
 */
export function groupCommitBodySections(blocks: readonly CommitBodyBlock[]): CommitBodySection[] {
  const sections: CommitBodySection[] = []
  // the section that unlabelled blocks join
  let current: CommitBodySection | null = null

  for (const [index, block] of blocks.entries()) {
    let label: string | null = null
    let content: CommitBodyBlock | null = block
    if (block.type === "paragraph") {
      const [first = "", ...others] = block.lines
      const leadIn = others.length === 0 ? LEAD_IN_PARAGRAPH.exec(first) : null
      const runIn = RUN_IN_LABEL.exec(first)
      // a lead-in paragraph needs blocks to introduce
      if (leadIn != null && blocks[index + 1] !== undefined && blocks[index + 1]!.type !== "trailers") {
        label = leadIn[1]!
        content = null
      } else if (runIn != null && runIn[1]!.length <= RUN_IN_LABEL_MAX_LENGTH) {
        label = runIn[1]!
        content = { type: "paragraph", lines: [runIn[2]!, ...others] }
      }
    }

    if (label != null || current == null || block.type === "trailers") {
      current = { label, blocks: [] }
      sections.push(current)
    }
    if (content != null) {
      current.blocks.push(content)
    }
  }
  return sections
}
