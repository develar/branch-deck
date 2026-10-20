/**
 * Which links are of the YouTrack the user signed in to, and which YouTrack to offer for a sign-in
 * (spec/repository/youtrack.spec.md). Nothing names a YouTrack: the candidates come from the issue links of
 * `.idea/vcs.xml`, the key from the link.
 */
import type { IssueNavigationConfig } from "@/lib/bindings"

/** A YouTrack to offer, and the repository whose `.idea/vcs.xml` links to it */
export interface YouTrackCandidate {
  url: string
  repository: string
}

/** `https://youtrack.example.com/issue/$0` (any group): the issue page of a YouTrack */
const ISSUE_LINK = /^(https?:\/\/[^?#]+?)\/issue\/\$\d+$/

const ISSUE_KEY = /^[A-Za-z][A-Za-z0-9_]*-\d+$/

/** The YouTracks the issue links of the recent repositories point to, most recent repository first, each once */
export function suggestYouTrackUrls(projects: readonly { path: string; issueNavigationConfig?: IssueNavigationConfig | undefined }[]): YouTrackCandidate[] {
  const candidates: YouTrackCandidate[] = []
  for (const project of projects) {
    for (const link of project.issueNavigationConfig?.links ?? []) {
      const url = ISSUE_LINK.exec(link.linkRegexp.trim())?.[1]
      if (url !== undefined && !candidates.some(it => it.url === url)) {
        candidates.push({ url, repository: project.path.split(/[\\/]/).findLast(part => part !== "") ?? project.path })
      }
    }
  }
  return candidates
}

/** The issue key of a link to `<base>/issue/<key>`; null for a link elsewhere (another host, another path) */
export function youTrackKeyOf(href: string, base: string): string | null {
  let link: URL
  let youtrack: URL
  try {
    link = new URL(href)
    youtrack = new URL(base)
  } catch {
    return null
  }
  if (link.origin !== youtrack.origin) {
    return null
  }
  const prefix = `${youtrack.pathname.replace(/\/$/, "")}/issue/`
  if (!link.pathname.startsWith(prefix)) {
    return null
  }
  let key: string
  try {
    key = decodeURIComponent(link.pathname.slice(prefix.length))
  } catch {
    return null
  }
  return ISSUE_KEY.test(key) ? key : null
}

/** Where a user creates and revokes tokens: Account Security of the YouTrack profile */
export function youTrackTokensPage(base: string): string {
  return `${base.replace(/\/$/, "")}/users/me?tab=account-security`
}

/** `youtrack.example.com`, `example.com/youtrack`: a YouTrack in a sentence */
export function formatYouTrackHost(base: string): string {
  return base.replace(/^https?:\/\//, "").replace(/\/$/, "")
}
