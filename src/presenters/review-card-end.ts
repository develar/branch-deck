// The words of the line at the end of a long card of a review and of its last card (spec/commit/commit-review.spec.md):
// what J shows next, which the review decides (`stepSection` of CommitReview), or that nothing is left.

/** The line at the end of a card: the section J shows next, or the end */
export type CardEnd =
  | {
      readonly kind: "next"
      /** The name of its file, bold */
      readonly name: string
      /** The last folder of its file, muted after the name; null for a file at the root and for `continued` */
      readonly folder: string | null
      /** The next section is a part of the same file: "Next: SKILL.md, continued" */
      readonly continued: boolean
      /** The accessible name of the line, which a click shows: "Next: SKILL.md setup-local-rd-flow" */
      readonly label: string
    }
  | { readonly kind: "end"; readonly text: string }

/**
 * The line at the end of the card of the section of `path`: the file J shows next, its name and its last folder (ten
 * files named SKILL.md in a row differ by it), "continued" when it is the next part of the same file; with nothing
 * left, "End of the review", or before the last card, under an owner chip whose files J steps through, the end of
 * them. Never "End of <file>": a section of the story may be a part of its file
 */
export function presentCardEnd(path: string, next: string | undefined, owner: string | null): CardEnd {
  if (next === undefined) {
    return { kind: "end", text: owner === null ? "End of the review" : `No more files of ${owner}` }
  }
  const slash = next.lastIndexOf("/")
  const name = next.slice(slash + 1)
  if (next === path) {
    return { kind: "next", name, folder: null, continued: true, label: `Next: ${name}, continued` }
  }
  const folder = slash < 0 ? null : next.slice(next.lastIndexOf("/", slash - 1) + 1, slash)
  return { kind: "next", name, folder, continued: false, label: folder === null ? `Next: ${name}` : `Next: ${name} ${folder}` }
}
