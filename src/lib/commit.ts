import type { Commit, MissingCommit } from "@/lib/bindings"
import type { SyncedCommit } from "@/stores/branch-sync"

/** Any commit shape displayed by the commit list. */
export type CommitUnion = Commit | SyncedCommit | MissingCommit

/** Original hash for `Commit`/`SyncedCommit`, `hash` for `MissingCommit`. */
export function getCommitHash(commit: CommitUnion): string {
  if ("originalHash" in commit && commit.originalHash) {
    return commit.originalHash
  }
  if ("hash" in commit && commit.hash) {
    return commit.hash
  }
  return ""
}

/** Subject line to display: stripped subject (without the grouping prefix), else the subject, else the whole message. */
export function getCommitSubject(commit: CommitUnion): string {
  if ("strippedSubject" in commit && commit.strippedSubject) {
    return commit.strippedSubject
  }
  if ("subject" in commit && commit.subject) {
    return commit.subject
  }
  return commit.message
}

/** Body of a multi-line message: everything after the subject line, without the blank lines that follow it. */
export function getCommitMessageBody(message: string): string {
  const lines = message.split("\n")
  let bodyStartIndex = 1
  while (bodyStartIndex < lines.length && lines[bodyStartIndex]?.trim() === "") {
    bodyStartIndex++
  }
  return lines.slice(bodyStartIndex).join("\n").trim()
}

/** Whether the message has a body beyond its subject line. */
export function hasCommitMessageBody(subject: string, message: string): boolean {
  return !!message && message !== subject && message.includes("\n")
}

// conventional-commit prefix: "feat:", "feat(model):", "fix!:", or a type and one scope word ("docs air:")
const SUBJECT_PREFIX = /^([a-z][a-z0-9-]*(?:\([^)]+\)| [a-z][\w./-]*)?!?:)\s+(\S.*)$/
// Branch Deck grouping prefix of a commit that is not stripped by the backend (missing commits): "(feature-auth) …"
const GROUPING_PREFIX = /^\(([^()\s]+)\)\s+(\S.*)$/

/** A subject with its prefix split off; `prefix` is null if there is none */
export interface SubjectParts {
  prefix: string | null
  text: string
}

/**
 * Splits a prefix off the subject: a grouping prefix (`(feature-auth) …`, returned without the parentheses) or a
 * conventional-commit prefix (with its colon). `prefix` is null if there is none.
 */
export function splitSubjectPrefix(subject: string): SubjectParts {
  const match = GROUPING_PREFIX.exec(subject) ?? SUBJECT_PREFIX.exec(subject)
  return match == null ? { prefix: null, text: subject } : { prefix: match[1]!, text: match[2]! }
}

/** "1 commit", "3 commits" */
export function formatCommitCount(count: number): string {
  return count === 1 ? "1 commit" : `${count} commits`
}

/** Number of changed files, if the commit shape carries it. */
export function getCommitFileCount(commit: CommitUnion): number | undefined {
  if ("fileDiffs" in commit && commit.fileDiffs?.length) {
    return commit.fileDiffs.length
  }
  if ("fileCount" in commit && typeof commit.fileCount === "number") {
    return commit.fileCount
  }
  return undefined
}

/** The branch a commit message groups into, mirroring the backend `CommitGrouper`. */
export interface CommitGroup {
  /** The branch; with stacked branches the leaf of `(parent/leaf)` */
  branch: string
  /** An explicit `(branch)` subject prefix, else the issue group of the subject's issue key, else that key */
  origin: "prefix" | "issueGroup" | "issue"
  /** With stacked branches: the parent the prefix declares, null for none; absent without stacked branches */
  parent?: string | null
  /** With stacked branches: the prefix has an empty level (`(a//b)`) */
  invalid?: boolean
}

// git's autosquash markers need the space: `fixup!X` is an ordinary subject
const AUTOSQUASH_MARKERS = ["fixup! ", "squash! ", "amend! "]

/** The subject after its leading autosquash markers, each with the whitespace after it (`fixup! squash! X` → `X`) */
function stripAutosquashMarkers(subject: string): string {
  let rest = subject
  for (;;) {
    const marker = AUTOSQUASH_MARKERS.find(it => rest.startsWith(it))
    if (marker === undefined) {
      return rest
    }
    rest = rest.slice(marker.length).trimStart()
  }
}

/**
 * Splits a subject like the backend `parse_subject`: the git autosquash markers, then a non-empty `(prefix)` at the
 * very start (unsanitized), then the rest.
 */
export function parseSubject(subject: string): { autosquash: string, prefix: string | null, rest: string } {
  const body = stripAutosquashMarkers(subject)
  const autosquash = subject.slice(0, subject.length - body.length)
  const closeParen = body.startsWith("(") ? body.indexOf(")") : -1
  if (closeParen > 1) {
    return { autosquash, prefix: body.slice(1, closeParen), rest: body.slice(closeParen + 1).trimStart() }
  }
  return { autosquash, prefix: null, rest: body }
}

/** The first line of a message */
export function getSubjectLine(message: string): string {
  return message.trimStart().split("\n", 1)[0]!
}

// as `find_issue_range`: an optional `[category]`, an optional `type(scope):`, then `ABC-123` at a word boundary
const ISSUE_KEY = /^(?:\[[^\]\n]*\]\s*)?(?:[a-z]+(?:\([^)\n]*\))?:\s*)?([A-Z]+-\d+)(?![A-Za-z0-9])/

/** The issue key a subject starts with (`ABC-123` in `[ui] fix(api): ABC-123 …`), or null. */
export function findIssueKey(subject: string): string | null {
  return ISSUE_KEY.exec(subject)?.[1] ?? null
}

/** A valid Git ref name part, as the backend `sanitize_branch_name`: `mockito 5.19` becomes `mockito-5.19`. */
export function sanitizeBranchName(name: string): string {
  return name
    .replace(/^\.+|\.+$/g, "")
    .replace(/^\/+|\/+$/g, "")
    .replace(/[ ~^:?*[\]\\]/g, "-")
    .replace(/-{2,}/g, "-")
    .replace(/^-|-$/g, "")
}

/**
 * The levels of a stack prefix as the backend `stack_path` reads them: sanitized, the empty ones at the ends dropped.
 * `invalid` for an empty level in the middle; `levels` is empty for a prefix without any branch name (`(/)`).
 */
function stackLevels(prefix: string): { levels: string[], invalid: boolean } {
  const levels = prefix.split("/").map(level => sanitizeBranchName(level.trim()))
  const first = levels.findIndex(level => level !== "")
  if (first === -1) {
    return { levels: [], invalid: true }
  }
  const last = levels.findLastIndex(level => level !== "")
  const path = levels.slice(first, last + 1)
  return path.includes("") ? { levels: [path.at(-1)!], invalid: true } : { levels: path, invalid: false }
}

/**
 * The branch the message groups into, or null: the commit is unassigned. Only the subject line counts. With `stacks`,
 * `(auth/login)` groups into `login` with the parent `auth`. `issueGroups` (issue key -> branch name, from the
 * `branchesGrouped` event) puts a subject without a prefix into the issue group of its key.
 */
export function groupFor(message: string, stacks = false, issueGroups: Readonly<Record<string, string>> = {}): CommitGroup | null {
  const { prefix, rest } = parseSubject(getSubjectLine(message))
  if (prefix !== null && stacks) {
    const { levels, invalid } = stackLevels(prefix)
    if (levels.length === 0) {
      return null
    }
    return { branch: levels.at(-1)!, origin: "prefix", parent: invalid ? null : (levels.at(-2) ?? null), ...(invalid && { invalid }) }
  }
  if (prefix !== null) {
    return { branch: sanitizeBranchName(prefix.trim()), origin: "prefix" }
  }
  const issueKey = findIssueKey(rest)
  if (issueKey === null) {
    return null
  }
  const issueGroup = issueGroups[issueKey]
  const group: CommitGroup = issueGroup === undefined ? { branch: issueKey, origin: "issue" } : { branch: issueGroup, origin: "issueGroup" }
  return stacks ? { ...group, parent: null } : group
}
