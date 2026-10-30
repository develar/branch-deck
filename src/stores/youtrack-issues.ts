import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { YouTrackAuthStatus, YouTrackIssueAnswer } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { compileIssuePatterns, parseTextSegments } from "@/lib/issue-links"
import { youTrackKeyOf } from "@/lib/youtrack"
import { refreshWindowAuth } from "@/model/auth-facts"
import { authStore } from "@/stores/auth"
import { getRecentProjects } from "@/stores/settings/repositorySettings"

/*
 * The issues of the popovers of a window (spec/repository/youtrack.spec.md), from the YouTrack the sign-in of the app
 * is for (the status of `src/stores/auth.ts`): an issue is asked when a popover shows it, its answer kept until the next
 * one, all of them forgotten once the YouTrack sign-in changed.
 */

/** An issue of a popover: asked when a popover shows it, the answer kept until the next one */
export type IssueEntry = YouTrackIssueAnswer

interface YouTrackIssuesState {
  /** By issue key */
  readonly issues: Readonly<Record<string, IssueEntry>>
}

export const youTrackIssuesStore = createStore<YouTrackIssuesState>(() => ({ issues: {} }))

/**
 * The sign-in an answer is of: another YouTrack, user or token answers anew; a problem the YouTrack found with the same
 * token keeps the answers (the popover that found it says it)
 */
function signInOf(status: YouTrackAuthStatus | undefined): string | null {
  return status === undefined ? null : JSON.stringify([status.url, status.signedInAs?.login, status.source, status.signedInAt])
}

// a sign-in or a sign-out in any window: the answers of the one before go
authStore.subscribe((state, before) => {
  if (signInOf(state.status?.youtrack) !== signInOf(before.status?.youtrack)) {
    youTrackIssuesStore.setState({ issues: {} })
  }
})

export function useIssueEntry(key: string): IssueEntry | undefined {
  return useStore(youTrackIssuesStore, state => state.issues[key])
}

/** The keys asked now */
const loadingIssues = new Set<string>()

/**
 * Asks the backend for the issue (it keeps an answer a minute); a second ask while one runs waits for it. A sign-in
 * problem the answer reports is the status of every surface of the window: it reads the sign-ins again
 */
export async function loadIssue(key: string): Promise<void> {
  if (loadingIssues.has(key)) {
    return
  }
  loadingIssues.add(key)
  const result = await commands.getYoutrackIssue(key).finally(() => loadingIssues.delete(key))
  const answer: YouTrackIssueAnswer = result.status === "ok" ? result.data : { status: "error", message: result.error.message }
  youTrackIssuesStore.setState(state => ({ issues: { ...state.issues, [key]: answer } }))
  if (answer.status === "signInProblem") {
    void refreshWindowAuth()
  }
}

/** The key of the branch's issue when it is an issue of the YouTrack signed in to, by the issue links of the repository */
export function getYouTrackKeyOfIssue(issueKey: string | null): string | null {
  const url = authStore.getState().status?.youtrack.url
  if (issueKey === null || url == null) {
    return null
  }
  const patterns = compileIssuePatterns(getRecentProjects()[0]?.issueNavigationConfig)
  const link = parseTextSegments(issueKey, patterns).find(segment => segment.type === "link")
  return link?.href === undefined ? null : youTrackKeyOf(link.href, url)
}

/** The value Mark Fixed sets: the resolved state of YouTrack's default workflow */
export const FIXED_STATE = "Fixed"
