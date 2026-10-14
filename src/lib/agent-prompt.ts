import type { UpdateCheck } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { groupConflictFiles } from "@/lib/update-conflict"

export interface AgentPromptInput {
  repositoryPath: string
  /** A conflicting or blocked update check */
  check: UpdateCheck
  /** When the upstream was fetched (ms), if known */
  fetchTime: number | null
}

/** Rules every hand-off states: Branch Deck must find the same commits, grouped the same way, afterwards */
const RULES = [
  "Keep every commit message unchanged, including the `(prefix)` at the start of a subject: Branch Deck groups commits by it.",
  "Don't squash, reorder, split or drop commits.",
  "Don't push.",
]

function bullets(lines: readonly string[]): string {
  return lines.map(line => `- ${line}`).join("\n")
}

function numbered(lines: readonly string[]): string {
  return lines.map((line, index) => `${index + 1}. ${line}`).join("\n")
}

function stateLines(check: UpdateCheck, fetchTime: number | null): string {
  const fetched = fetchTime === null ? "" : ` (fetched ${new Date(fetchTime).toISOString()})`
  return bullets([
    `HEAD (${check.branch}): ${check.head}`,
    `${check.upstream}${fetched}: ${check.upstreamHead}`,
    `Merge base: ${check.base}`,
  ])
}

/**
 * The prompt for an external coding agent that does the update Branch Deck refused: a rebase with conflicts, or one
 * blocked by uncommitted changes on files that the upstream changed. Copied to the clipboard; the agent runs git in a
 * terminal, so it is told the exact state and what it must not change.
 */
export function buildAgentPrompt({ repositoryPath, check, fetchTime }: AgentPromptInput): string {
  const { state, branch, upstream } = check
  const sections: string[] = []
  if (state.status === "conflict") {
    sections.push(
      `Rebase the branch \`${branch}\` of the Git repository at ${repositoryPath} onto \`${upstream}\` and resolve the conflicts.`,
      stateLines(check, fetchTime),
      `My local commits, oldest first:\n${bullets(state.commits.map(commit => `${formatShortHash(commit.id)} ${commit.subject}`))}`,
    )
    if (state.files.length > 0) {
      const files = groupConflictFiles(state.files, state.commits, state.upstreamCommits).map(({ path, local, upstream }) => {
        const sides = [
          ...(local.length === 0 ? [] : [`changed by my ${local.map(commit => formatShortHash(commit.id)).join(", ")}`]),
          ...upstream.map(commit => `changed upstream by ${formatShortHash(commit.id)} ${commit.subject} (${commit.author})`),
        ]
        return [path, ...sides.map(side => `  - ${side}`)].join("\n")
      })
      sections.push(`Files that likely conflict:\n${bullets(files)}`)
    }
    sections.push(
      `Rules:\n${bullets([
        ...RULES,
        "If `git status` shows uncommitted changes, run `git stash push --include-untracked` first and `git stash pop` at the end.",
      ])}`,
      `Steps:\n${numbered([
        `\`git rebase ${upstream}\`.`,
        "Resolve each conflict: keep the upstream change and reapply the intent of my commit on top of it.",
        "Build and run the tests of the code you touched.",
        "`git add` the resolved files and `git rebase --continue`, until the rebase is done.",
        "Report each resolution: the file, the commit, and what you kept from each side.",
      ])}`,
    )
  }
  else if (state.status === "blocked") {
    sections.push(
      `Update the branch \`${branch}\` of the Git repository at ${repositoryPath} to \`${upstream}\`. My uncommitted changes in files that the upstream also changed block it.`,
      stateLines(check, fetchTime),
      `My uncommitted changes that overlap the upstream:\n${bullets(state.paths)}`,
      `Rules:\n${bullets([
        ...RULES,
        "Keep my uncommitted changes uncommitted: restore them on top of the updated files.",
      ])}`,
      `Steps:\n${numbered([
        "`git stash push --include-untracked`. A listed file that git ignores is not stashed: move it out of the repository and back afterwards.",
        `\`git rebase ${upstream}\`; resolve any conflict by keeping the upstream change and reapplying the intent of my commit.`,
        "`git stash pop`. On a conflict, merge my change with the upstream one, keep the result uncommitted, and `git stash drop` only once it is restored.",
        "Build and run the tests of the code you touched.",
        "Report what you merged in each file.",
      ])}`,
    )
  }
  return sections.join("\n\n")
}
