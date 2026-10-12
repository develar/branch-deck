import { Fragment, useMemo } from "react"
import { cn } from "cn"
import { splitInlineCode } from "@/lib/inline-code"
import type { InlineSegment } from "@/lib/inline-code"
import { parseTextSegments, useIssuePatterns } from "@/lib/issue-links"
import type { CompiledIssuePattern } from "@/lib/issue-links"

export interface LinkedTextProps {
  text: string
  className?: string
  /** Render code tokens (backtick spans, identifiers, paths, flags) in the monospace font; see `splitInlineCode` */
  detectCode?: boolean
}

/** Text with issue references (per the project's issue navigation config) rendered as external links. */
export function LinkedText({ text, className, detectCode = false }: LinkedTextProps) {
  const patterns = useIssuePatterns()
  const content = useMemo(() => {
    if (!detectCode) {
      return renderLinks(text, patterns)
    }
    // code first: a path or identifier that contains an issue key stays one code token
    return splitInlineCode(text).map((segment, index) => (
      <Fragment key={index}>
        {segment.type === "code" ? <InlineCode segment={segment} /> : renderLinks(segment.content, patterns)}
      </Fragment>
    ))
  }, [text, patterns, detectCode])

  return <span className={className}>{content}</span>
}

function renderLinks(text: string, patterns: readonly CompiledIssuePattern[]) {
  const segments = parseTextSegments(text, patterns)
  if (segments.length === 1 && segments[0]?.type === "text") {
    return segments[0].content
  }

  // segments of the same text never reorder, so the index is a stable key
  return segments.map((segment, index) => segment.type === "link"
    ? (
        <a
          key={index}
          href={segment.href}
          target="_blank"
          rel="noopener noreferrer"
          className="rounded-md text-muted-foreground transition-colors outline-primary/25 hover:text-foreground focus-visible:outline-3"
        >
          {segment.content}
        </a>
      )
    : <span key={index}>{segment.content}</span>)
}

// break opportunities in long identifiers and paths: before `/`, `_` and a single `.` (not inside Bazel's `...`)
const CODE_BREAK = /(?=\/|_|(?<!\.)\.(?!\.))/

function InlineCode({ segment }: { segment: InlineSegment }) {
  const parts = segment.content.split(CODE_BREAK)
  return (
    <code
      data-slot="inline-code"
      data-explicit={segment.explicit ? "" : undefined}
      className={cn(
        "font-mono text-[0.9em] leading-none text-foreground [overflow-wrap:anywhere]",
        segment.explicit && "rounded-sm bg-muted px-1",
      )}
    >
      {parts.map((part, index) => (
        <Fragment key={index}>
          {index > 0 && <wbr />}
          {part}
        </Fragment>
      ))}
    </code>
  )
}
