/**
 * Converts issue references in text to links, based on the IntelliJ IDEA issue navigation configuration
 * (`.idea/vcs.xml`) of the selected project.
 */
import type { IssueNavigationConfig } from "@/lib/bindings"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

export interface CompiledIssuePattern {
  regex: RegExp
  linkTemplate: string
}

export interface TextSegment {
  type: "text" | "link"
  content: string
  href?: string
}

const noPatterns: readonly CompiledIssuePattern[] = []
const compiledCache = new WeakMap<IssueNavigationConfig, readonly CompiledIssuePattern[]>()

/** Compiles the issue patterns of a config; cached per config object. */
export function compileIssuePatterns(config: IssueNavigationConfig | null | undefined): readonly CompiledIssuePattern[] {
  if (!config?.links) {
    return noPatterns
  }

  const cached = compiledCache.get(config)
  if (cached !== undefined) {
    return cached
  }

  const compiled: CompiledIssuePattern[] = []
  for (const link of config.links) {
    try {
      // IntelliJ uses Java regex syntax; `\b` works the same in JavaScript
      compiled.push({ regex: new RegExp(link.issueRegexp, "g"), linkTemplate: link.linkRegexp })
    } catch (error) {
      console.warn("Failed to compile issue pattern:", link.issueRegexp, error)
    }
  }
  compiledCache.set(config, compiled)
  return compiled
}

interface IssueMatch {
  start: number
  end: number
  href: string
  content: string
}

/** Splits text into plain text and link segments. Earlier patterns win on overlapping matches. */
export function parseTextSegments(text: string, patterns: readonly CompiledIssuePattern[]): TextSegment[] {
  if (!text || patterns.length === 0) {
    return [{ type: "text", content: text }]
  }

  const matches: IssueMatch[] = []
  for (const pattern of patterns) {
    pattern.regex.lastIndex = 0

    let match: RegExpExecArray | null
    while ((match = pattern.regex.exec(text)) !== null) {
      const fullMatch = match[0]
      // guard against zero-length matches looping forever
      if (fullMatch.length === 0) {
        pattern.regex.lastIndex++
        continue
      }

      const start = match.index
      const end = start + fullMatch.length
      const overlaps = matches.some(it => (start >= it.start && start < it.end) || (end > it.start && end <= it.end))
      if (overlaps) {
        continue
      }

      // $0 is the full match, $1, $2, ... are the capture groups
      let href = pattern.linkTemplate.replace(/\$0/g, encodeURIComponent(fullMatch))
      for (let i = 1; i < match.length; i++) {
        href = href.replace(new RegExp(`\\$${i}`, "g"), encodeURIComponent(match[i] || ""))
      }
      matches.push({ start, end, href, content: fullMatch })
    }
  }

  if (matches.length === 0) {
    return [{ type: "text", content: text }]
  }

  matches.sort((a, b) => a.start - b.start)

  const segments: TextSegment[] = []
  let lastEnd = 0
  for (const match of matches) {
    if (match.start > lastEnd) {
      segments.push({ type: "text", content: text.substring(lastEnd, match.start) })
    }
    segments.push({ type: "link", content: match.content, href: match.href })
    lastEnd = match.end
  }
  if (lastEnd < text.length) {
    segments.push({ type: "text", content: text.substring(lastEnd) })
  }
  return segments
}

/** Whether the text contains at least one issue reference. */
export function hasIssueReferences(text: string, patterns: readonly CompiledIssuePattern[]): boolean {
  if (!text) {
    return false
  }
  return patterns.some(pattern => {
    pattern.regex.lastIndex = 0
    return pattern.regex.test(text)
  })
}

/** Compiled issue patterns of the selected project; re-renders only when its issue navigation config changes. */
export function useIssuePatterns(): readonly CompiledIssuePattern[] {
  const config = useRecentProjects(projects => projects[0]?.issueNavigationConfig as IssueNavigationConfig | undefined)
  return compileIssuePatterns(config)
}
