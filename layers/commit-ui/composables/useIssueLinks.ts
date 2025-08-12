/**
 * Composable for processing text with issue navigation patterns
 * Converts issue references to clickable links based on IntelliJ IDEA configuration
 */

import type { IssueNavigationLink } from "~/utils/bindings"

interface CompiledPattern {
  regex: RegExp
  linkTemplate: string
}

export interface TextSegment {
  type: "text" | "link"
  content: string
  href?: string
}

export function useIssueLinks() {
  const { selectedProject } = useRepository()

  // Cached compiled patterns
  const compiledPatterns = computed<CompiledPattern[]>(() => {
    const config = selectedProject.value?.issueNavigationConfig
    if (!config?.links) {
      return []
    }

    return config.links.map((link: IssueNavigationLink) => {
      try {
        // Convert the issue regexp to a JavaScript RegExp
        // IntelliJ uses Java regex syntax, we need to handle some differences
        let pattern = link.issueRegexp

        // Replace \b word boundaries if not already escaped
        pattern = pattern.replace(/\\b/g, "\\b")

        // Create the regex with global flag to find all matches
        const regex = new RegExp(pattern, "g")

        return {
          regex,
          linkTemplate: link.linkRegexp,
        }
      }
      catch (e) {
        console.warn("Failed to compile issue pattern:", link.issueRegexp, e)
        return null
      }
    }).filter(Boolean) as CompiledPattern[]
  })

  /**
   * Parse text into segments with links for issue references
   * @param text The text to parse
   * @returns Array of text segments
   */
  function parseTextSegments(text: string): TextSegment[] {
    if (!text || compiledPatterns.value.length === 0) {
      return [{ type: "text", content: text }]
    }

    // Track all matches to avoid overlaps
    interface Match {
      start: number
      end: number
      href: string
      content: string
    }

    const matches: Match[] = []

    // Find all matches for each pattern
    for (const pattern of compiledPatterns.value) {
      // Reset regex lastIndex
      pattern.regex.lastIndex = 0

      let match: RegExpExecArray | null
      while ((match = pattern.regex.exec(text)) !== null) {
        const fullMatch = match[0]
        const start = match.index
        const end = start + fullMatch.length

        // Check if this overlaps with existing matches
        const overlaps = matches.some(m =>
          (start >= m.start && start < m.end)
          || (end > m.start && end <= m.end),
        )

        if (!overlaps) {
          // Build the link URL by replacing $0, $1, etc. with capture groups
          let url = pattern.linkTemplate

          // Replace $0 with full match
          url = url.replace(/\$0/g, encodeURIComponent(fullMatch))

          // Replace $1, $2, etc. with capture groups
          for (let i = 1; i < match.length; i++) {
            const placeholder = new RegExp(`\\$${i}`, "g")
            url = url.replace(placeholder, encodeURIComponent(match[i] || ""))
          }

          matches.push({
            start,
            end,
            href: url,
            content: fullMatch,
          })
        }
      }
    }

    // If no matches, return the whole text as a single segment
    if (matches.length === 0) {
      return [{ type: "text", content: text }]
    }

    // Sort matches by start position
    matches.sort((a, b) => a.start - b.start)

    // Build segments
    const segments: TextSegment[] = []
    let lastEnd = 0

    for (const match of matches) {
      // Add text before this match
      if (match.start > lastEnd) {
        segments.push({
          type: "text",
          content: text.substring(lastEnd, match.start),
        })
      }

      // Add the link
      segments.push({
        type: "link",
        content: match.content,
        href: match.href,
      })

      lastEnd = match.end
    }

    // Add any remaining text after the last match
    if (lastEnd < text.length) {
      segments.push({
        type: "text",
        content: text.substring(lastEnd),
      })
    }

    return segments
  }

  /**
   * Process text and check if it contains any issue references
   * @param text The text to check
   * @returns True if text contains issue references
   */
  function hasIssueReferences(text: string): boolean {
    if (!text || compiledPatterns.value.length === 0) {
      return false
    }

    for (const pattern of compiledPatterns.value) {
      pattern.regex.lastIndex = 0
      if (pattern.regex.test(text)) {
        return true
      }
    }

    return false
  }

  return {
    parseTextSegments,
    hasIssueReferences,
    hasPatterns: computed(() => compiledPatterns.value.length > 0),
  }
}
