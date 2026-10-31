import type { Collapse, GeneratedName } from "@/lib/bindings"

/*
 * The words of why a file of a diff starts collapsed (spec/commit/commit-review.spec.md): Rust decides it
 * (`collapse_of`), this says it in the header of the file. A file without text says so in its notice, and a large one
 * shows its counts: their headers need no word.
 */

export const GENERATED_NAME_WORDS: Readonly<Record<GeneratedName, string>> = {
  lockFile: "Lock file",
  minified: "Minified",
  sourceMap: "Source map",
  snapshot: "Snapshot",
}

/** What the header of a collapsed file says of why: "Lock file"; null when its notice or its counts say it */
export function formatCollapse(collapse: Collapse | null): string | null {
  return collapse?.kind === "generated" ? GENERATED_NAME_WORDS[collapse.name] : null
}
