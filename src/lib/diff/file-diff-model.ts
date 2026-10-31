import type { Collapse, DiffContent, DiffFileHeader, DiffSource } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"

/*
 * The diff of a file as the backend streams it (`get_file_diffs`, the Specta bindings): typed blocks of the diff
 * engine, the texts apart by blob id, no patch text.
 */

export type { BlockKind, DiffBlock, DiffContent, LineEndKind, LineEnds, Scope, TextDiff, WhitespacePolicy } from "@/lib/bindings"

/** A changed file with its content, once the stream gave it, and why the views start it collapsed (Rust's `collapse_of`) */
export interface FileDiff extends DiffFileHeader {
  content: DiffContent
  collapse: Collapse | null
}

/**
 * What a diff shows: a commit, a range, or the uncommitted changes of the working tree of a repository (a commit and a
 * range are the same diff in every repository that has them, the working trees are not)
 */
export type FileDiffSubject = DiffSubject | { readonly kind: "uncommitted"; readonly repositoryPath: string }

/** The source of the backend for a subject */
export function diffSourceOf(subject: FileDiffSubject): DiffSource {
  switch (subject.kind) {
    case "commit":
      return { kind: "commit", id: subject.id }
    case "range":
      return { kind: "range", base: subject.base, head: subject.head }
    case "uncommitted":
      return { kind: "uncommitted" }
  }
}

/** The id of a subject in the stores: the commit id, `<base>..<head>`, or `uncommitted:<repository path>` */
export function fileDiffSubjectId(subject: FileDiffSubject): string {
  switch (subject.kind) {
    case "commit":
      return subject.id
    case "range":
      return `${subject.base}..${subject.head}`
    case "uncommitted":
      return `uncommitted:${subject.repositoryPath}`
  }
}
