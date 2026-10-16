import type { AgentCliCommand } from "@/lib/bindings"
import { bullets, numbered } from "@/lib/prompt-text"

export interface ConflictPromptInput {
  repositoryPath: string
  branch: string
  /** The commit that does not apply: its original id and subject */
  commit: { hash: string; subject: string }
  /** What the branch is built on */
  builtOn: string
  files: readonly string[]
  /** The explanation of the conflict view: where the missing commits are */
  explanation: string
  /** How this app's bd runs in the repository; null when the app can't tell its binary */
  bd: AgentCliCommand | null
}

/** A regroup rewrites prefixes, so the rules of the other hand-offs (keep every message) don't fit */
const CONFLICT_RULES = [
  "Don't edit, squash, reorder, split or drop commits yourself; a regroup changes only the `(prefix)` of subjects, the issue groups and the stack parents.",
  "Don't push.",
]

/** The most conflicting files the prompt names */
const SHOWN_FILES = 20

/**
 * The prompt that hands a sync conflict without a fix to a coding agent: what conflicts and why, then bd's trial builds
 * (`bd check --fix`, `bd check --plan`) to find a regroup or a hunk move, shown to the user before it is applied.
 */
export function buildConflictPrompt(input: ConflictPromptInput): string {
  const { repositoryPath, branch, commit, builtOn, files, explanation, bd } = input
  const shown = files.slice(0, SHOWN_FILES)
  const more = files.length - shown.length
  const fileLines = more > 0 ? [...shown, `… and ${more} more`] : shown
  const steps =
    bd === null
      ? [
          "Find the commits the change needs: the older commits that change the same lines of these files.",
          "Propose a regroup so that the change and what it needs are in one branch or in the branch it is built on: a `(branch)` prefix for a commit, or a stack parent for a branch. Show it to me before you rewrite anything.",
        ]
      : [
          `\`${bd.command} check --fix\`${bd.powershell ? " (in PowerShell)" : ""}: the trial builds of Branch Deck; \`${bd.command} help agent\` explains each field. Below, \`bd\` stands for this command.`,
          "No fix is found: find the commits the change needs (`bd commits`), and try plans with `bd check --plan plan.json` until the branch is `ok`: move the change and what it needs into one branch or into the branch it is built on, or move the change of a file from one commit into another with `bd move-hunk`.",
          "Show me the plan and its `bd check` result. Only when I approve: `bd group --expect-head <head> --sync --plan plan.json`.",
        ]
  return [
    `Fix the merge conflict of the virtual branch \`${branch}\` of the Git repository at ${repositoryPath}. Branch Deck builds the branch from my commits by their \`(prefix)\`, issue key or issue group, on \`${builtOn}\`.`,
    `The commit “${commit.subject}” (${commit.hash}) does not apply cleanly onto \`${builtOn}\`. ${explanation}. Branch Deck found no regroup that applies it cleanly.`,
    `Conflicting files:\n${bullets(fileLines)}`,
    `Steps:\n${numbered(steps)}`,
    `Rules:\n${bullets(CONFLICT_RULES)}`,
  ].join("\n\n")
}
