import { useMemo } from "react"
import { parseTextSegments, useIssuePatterns } from "@/lib/issue-links"

export interface LinkedTextProps {
  text: string
  className?: string
}

/** Text with issue references (per the project's issue navigation config) rendered as external links. */
export function LinkedText({ text, className }: LinkedTextProps) {
  const patterns = useIssuePatterns()
  const segments = useMemo(() => parseTextSegments(text, patterns), [text, patterns])

  if (segments.length === 1 && segments[0]?.type === "text") {
    return <span className={className}>{segments[0].content}</span>
  }

  // segments of the same text never reorder, so the index is a stable key
  return (
    <span className={className}>
      {segments.map((segment, index) => segment.type === "link"
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
        : <span key={index}>{segment.content}</span>)}
    </span>
  )
}
