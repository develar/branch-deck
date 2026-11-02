import { Fragment } from "react"
import type { ReactNode } from "react"
import { LinkedText } from "@/components/commit/LinkedText"
import { InlineCode } from "@/components/shared/InlineCode"
import type { CodeSpan } from "@/lib/bindings"
import { codeWritten, isCodeChip, splitCodeAtBreaks } from "@/lib/inline-code"

/**
 * A line of prose with its spans of code as Rust found them (`sync_utils::code_spans`: a commit body, the prose of a
 * spec): a span in backticks or of several words a chip without its backticks, a name the grammar found only the code
 * font; the prose between them with its issue links. `code` renders a span another way (the story links a name that
 * resolves): null keeps it code
 */
export function CodeText({ text, spans, code }: { text: string; spans: readonly CodeSpan[]; code?: (span: CodeSpan, written: string) => ReactNode }) {
  if (spans.length === 0) {
    return <LinkedText text={text} />
  }
  const parts: ReactNode[] = []
  let at = 0
  for (const span of spans) {
    if (span.start < at || span.end > text.length) {
      continue
    }
    if (span.start > at) {
      parts.push(<LinkedText key={`text:${at}`} text={text.slice(at, span.start)} />)
    }
    const written = codeWritten(text, span)
    parts.push(<Fragment key={`code:${span.start}`}>{code?.(span, written) ?? <CodeWords span={span} written={written} />}</Fragment>)
    at = span.end
  }
  if (at < text.length) {
    parts.push(<LinkedText key={`text:${at}`} text={text.slice(at)} />)
  }
  return <>{parts}</>
}

/** The words of a span of code, a long path breakable at its slashes and dots */
function CodeWords({ span, written }: { span: CodeSpan; written: string }) {
  const chip = isCodeChip(span)
  return (
    <InlineCode plain={!chip} data-explicit={span.explicit ? "" : undefined}>
      {splitCodeAtBreaks(written).map((part, index) => (
        <Fragment key={index}>
          {index > 0 && <wbr />}
          {part}
        </Fragment>
      ))}
    </InlineCode>
  )
}
