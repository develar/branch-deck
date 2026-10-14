import { Fragment, useMemo } from "react"
import { cn } from "cn"
import { InlineCode } from "@/components/shared/InlineCode"
import { splitCodeAtBreaks, splitInlineCode } from "@/lib/inline-code"
import type { InlineSegment } from "@/lib/inline-code"
import { parseTextSegments, useIssuePatterns } from "@/lib/issue-links"
import type { CompiledIssuePattern } from "@/lib/issue-links"

export interface LinkedTextProps {
  text: string
  className?: string
  /** Render code tokens (backtick spans, identifiers, paths, flags) in the monospace font; see `splitInlineCode` */
  detectCode?: boolean
  /** Colour of the issue links; muted by default, so prose links stay quiet */
  linkClassName?: string
}

/** Text with issue references (per the project's issue navigation config) rendered as external links. */
export function LinkedText({ text, className, detectCode = false, linkClassName = "text-muted-foreground" }: LinkedTextProps) {
  const patterns = useIssuePatterns()
  const content = useMemo(() => {
    if (!detectCode) {
      return renderLinks(text, patterns, linkClassName)
    }
    // code first: a path or identifier that contains an issue key stays one code token
    return splitInlineCode(text).map((segment, index) => (
      <Fragment key={index}>
        {segment.type === "code" ? <CodeSegment segment={segment} /> : renderLinks(segment.content, patterns, linkClassName)}
      </Fragment>
    ))
  }, [text, patterns, detectCode, linkClassName])

  return <span className={className}>{content}</span>
}

function renderLinks(text: string, patterns: readonly CompiledIssuePattern[], linkClassName: string) {
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
          className={cn("rounded-sm transition-colors outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30", linkClassName)}
        >
          {segment.content}
        </a>
      )
    : <span key={index}>{segment.content}</span>)
}

/** A backtick span is a chip; a detected identifier or path only gets the monospace font */
function CodeSegment({ segment }: { segment: InlineSegment }) {
  const parts = splitCodeAtBreaks(segment.content)
  return (
    <InlineCode plain={!segment.explicit} data-explicit={segment.explicit ? "" : undefined}>
      {parts.map((part, index) => (
        <Fragment key={index}>
          {index > 0 && <wbr />}
          {part}
        </Fragment>
      ))}
    </InlineCode>
  )
}
