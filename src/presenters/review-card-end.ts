// The words of the line at the end of a long card of a review and of its last card (spec/commit/commit-review.spec.md):
// what J shows next, which the review decides (`stepSection` of CommitReview), or that nothing is left.

/** The line at the end of a card: the section J shows next, or the end */
export type CardEnd =
  | {
      readonly kind: "next"
      /** The name of its file, bold */
      readonly name: string
      /**
       * The next section is a part of the same file: "Next: SKILL.md, continued"; else the line names its file as its
       * header does, the name and then the directory (`FilePathLabel`)
       */
      readonly continued: boolean
      /** The accessible name of the line, which a click shows: "Next: SKILL.md .claude/skills/setup-local-rd-flow" */
      readonly label: string
    }
  | { readonly kind: "end"; readonly text: string }

/**
 * The line at the end of the card of the section of `path`: the file J shows next, by its name and its directory as
 * the header of its card names it (ten files named SKILL.md in a row differ by their last folder), "continued" when it
 * is the next part of the same file; with nothing left, "End of the review", or before the last card, under an owner
 * chip whose files J steps through, the end of them. Never "End of <file>": a section of the story may be a part of
 * its file
 */
export function presentCardEnd(path: string, next: string | undefined, owner: string | null): CardEnd {
  if (next === undefined) {
    return { kind: "end", text: owner === null ? "End of the review" : `No more files of ${owner}` }
  }
  const slash = next.lastIndexOf("/")
  const name = next.slice(slash + 1)
  if (next === path) {
    return { kind: "next", name, continued: true, label: `Next: ${name}, continued` }
  }
  return { kind: "next", name, continued: false, label: slash < 0 ? `Next: ${name}` : `Next: ${name} ${next.slice(0, slash)}` }
}
