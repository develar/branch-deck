import { Fragment } from "react"
import type { ReactNode } from "react"
import { LinkedText } from "@/components/commit/LinkedText"
import { InlineCode } from "@/components/shared/InlineCode"
import type { CodeSpan } from "@/lib/bindings"
import { codeWritten, glueAfter, glueBefore, isCodeChip, splitCodeAtBreaks } from "@/lib/inline-code"

/**
 * A line of prose with its spans of code as Rust found them (`sync_utils::code_spans`: a commit body, the prose of a
 * spec): a span in backticks or of several words a chip without its backticks, a name the grammar found only the code
 * font; the prose between them with its issue links. `code` renders a span another way (the story links a name that
 * resolves): null keeps it code. The punctuation that touches a span stays on its line (`CodeGlue`)
 */
export function CodeText({ text, spans, code }: { text: string; spans: readonly CodeSpan[]; code?: (span: CodeSpan, written: string) => ReactNode }) {
  if (spans.length === 0) {
    return <LinkedText text={text} />
  }
  const parts: ReactNode[] = []
  let at = 0
  for (const [index, span] of spans.entries()) {
    if (span.start < at || span.end > text.length) {
      continue
    }
    const before = glueBefore(text.slice(at, span.start))
    if (span.start - before.length > at) {
      parts.push(<LinkedText key={`text:${at}`} text={text.slice(at, span.start - before.length)} />)
    }
    // the prose up to the next span: the punctuation it starts with is glued to this span
    const next = spans.slice(index + 1).find(it => it.start >= span.end)?.start ?? text.length
    const after = glueAfter(text.slice(span.end, next))
    const written = codeWritten(text, span)
    parts.push(
      <CodeGlue key={`code:${span.start}`} before={before} after={after}>
        {code?.(span, written) ?? <CodeWords span={span} written={written} />}
      </CodeGlue>,
    )
    at = span.end + after.length
  }
  if (at < text.length) {
    parts.push(<LinkedText key={`text:${at}`} text={text.slice(at)} />)
  }
  return <>{parts}</>
}

/**
 * Code and the punctuation that touches it, one unit of a line: "(Frame.paint)," never breaks before its comma or after
 * its parenthesis (a link of code is a button, an atomic inline a line may break around). The code inside breaks as it
 * would alone, at its slashes and dots (`BreakableCode`)
 */
export function CodeGlue({ before = "", after = "", children }: { before?: string; after?: string; children: ReactNode }) {
  if (before === "" && after === "") {
    return <>{children}</>
  }
  return (
    <span className="whitespace-nowrap">
      {before}
      <span className="whitespace-normal">{children}</span>
      {after}
    </span>
  )
}

/** A name or a path of code, breakable only at its own boundaries (`splitCodeAtBreaks`): `Terminal.rightEdge`, never `Termi`/`nal` */
export function BreakableCode({ text }: { text: string }) {
  return splitCodeAtBreaks(text).map((part, index) => (
    <Fragment key={index}>
      {index > 0 && <wbr />}
      {part}
    </Fragment>
  ))
}

/** The words of a span of code, a long path breakable at its slashes and dots */
function CodeWords({ span, written }: { span: CodeSpan; written: string }) {
  return (
    <InlineCode plain={!isCodeChip(span)} data-explicit={span.explicit ? "" : undefined}>
      <BreakableCode text={written} />
    </InlineCode>
  )
}
