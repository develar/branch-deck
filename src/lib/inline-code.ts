import type { CodeSpan } from "@/lib/bindings"

/** The text of a span without its backticks */
export function codeWritten(text: string, span: CodeSpan): string {
  const source = text.slice(span.start, span.end)
  return span.explicit ? source.slice(1, -1) : source
}

/** A span of code is a chip when the author marked it or Rust joined several tokens into it; a name the grammar found is only mono */
export function isCodeChip(span: CodeSpan): boolean {
  return span.explicit || span.joined
}

// break opportunities in long paths and dotted names: before a `/` or `.` that follows a name character (not inside
// `//`, `./` or Bazel's `...`), and not before a short final extension (`Foo` / `.kt`); CONSTANT_CASE names stay whole
const CODE_BREAK = /(?<=[^/.])(?=\/)|(?<=[^.])(?=\.(?![./]|\w{1,4}$))/

/** Splits a code token at its line-break opportunities: `src/lib/commit.ts` → `src/`, `lib/`, `commit.ts`. */
export function splitCodeAtBreaks(content: string): string[] {
  return content.split(CODE_BREAK)
}

/** A part of a name past which a line of the sidebar or of a column may not hold it: its humps are its breaks too */
const LONG_PART = 24

// a hump of camelCase: a capital after a lowercase letter or a digit
const HUMP = /(?<=[\p{Ll}\d])(?=\p{Lu})/u

/**
 * The parts of a code token a line may break between: at its slashes and dots (`splitCodeAtBreaks`), and inside a part
 * of `LONG_PART` characters or more at its humps (`TerminalContent`, `Updated`, `Event`), so a name wider than its line
 * breaks between words, never inside one; a shorter part stays whole, so a dot is the break while one exists
 */
export function codeBreakParts(content: string): string[] {
  return splitCodeAtBreaks(content).flatMap(part => (part.length >= LONG_PART ? part.split(HUMP) : [part]))
}

// the punctuation that touches code and must not start a line without it (closing marks, the end of a sentence) or end
// a line before it (opening marks)
const GLUE_AFTER = /^[,.;:!?)\]}”’»…]+/
const GLUE_BEFORE = /[([{“‘«]+$/

/** The punctuation the prose after a span of code starts with, which stays on the line of the code: `), ` → `),` */
export function glueAfter(prose: string): string {
  return GLUE_AFTER.exec(prose)?.[0] ?? ""
}

/** The opening marks the prose before a span of code ends with, which stay on the line of the code: ` (` → `(` */
export function glueBefore(prose: string): string {
  return GLUE_BEFORE.exec(prose)?.[0] ?? ""
}
