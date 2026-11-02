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
