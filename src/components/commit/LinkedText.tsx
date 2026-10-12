import { Fragment, useMemo } from "react"
import { cn } from "cn"
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
        {segment.type === "code" ? <InlineCode segment={segment} /> : renderLinks(segment.content, patterns, linkClassName)}
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
          className={cn("rounded-md transition-colors outline-primary/25 hover:text-foreground focus-visible:outline-3", linkClassName)}
        >
          {segment.content}
        </a>
      )
    : <span key={index}>{segment.content}</span>)
}

function InlineCode({ segment }: { segment: InlineSegment }) {
  const parts = splitCodeAtBreaks(segment.content)
  return (
    <code
      data-slot="inline-code"
      data-explicit={segment.explicit ? "" : undefined}
      className={cn(
        "font-mono text-[0.875em] leading-none [overflow-wrap:anywhere]",
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
