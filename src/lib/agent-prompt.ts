import type { AgentCliCommand, UpdateCheck, UpdateCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { bullets, numbered, RULES, shellWord, UPDATE_RULES } from "@/lib/prompt-text"
import { cutList } from "@/lib/text"
import { groupConflictFiles } from "@/lib/update-conflict"

export interface AgentPromptInput {
  repositoryPath: string
  /** A conflicting or blocked update check */
  check: UpdateCheck
  /** When the upstream was fetched (ms), if known */
  fetchTime: number | null
  /** How this app's bd runs in the repository (`agent_cli_command`); null when the app can't tell its binary */
  bd: AgentCliCommand | null
}

/** The overlapping paths a blocked prompt lists; untracked files the upstream adds can be thousands */
const MAX_BLOCKED_PATHS = 50

type ConflictState = Extract<UpdateCheck["state"], { status: "conflict" }>
type BlockedState = Extract<UpdateCheck["state"], { status: "blocked" }>

/** `2026-10-14`, UTC, of Unix seconds */
function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toISOString().slice(0, 10)
}

/**
 * `cccccccc Change line 2 (Dana Lee, authored 2026-10-01, in origin/master since 2026-10-14)`: the committer time is
 * about when the commit landed in the upstream
 */
function upstreamCommitLine(commit: UpdateCommit, upstream: string): string {
  const dates = `authored ${formatDate(commit.time)}, in ${upstream} since ${formatDate(commit.committerTime)}`
  return `${formatShortHash(commit.id)} ${commit.subject} (${commit.author}, ${dates})`
}

function stateLines(check: UpdateCheck, fetchTime: number | null): string {
  const fetched = fetchTime === null ? "" : ` (fetched ${new Date(fetchTime).toISOString()})`
  return bullets([`HEAD (${check.branch}): ${check.head}`, `${check.upstream}${fetched}: ${check.upstreamHead}`, `Merge base: ${check.base}`])
}

/**
 * The blocked prompt: other sessions may work in the same tree, so the agent must not stash, reset, check out or
 * commit the files; it asks the user about each. Then bd updates, which writes only the files the update changes, or,
 * without a bd command, the user updates in Branch Deck. Never a stash: it would take the files of the other sessions
 */
function blockedPrompt(repositoryPath: string, check: UpdateCheck, state: BlockedState, bd: AgentCliCommand | null): string {
  const { branch, upstream } = check
  // the backend reports the first paths and counts all
  const { shown, more: rest } = cutList(state.paths, MAX_BLOCKED_PATHS, state.pathCount)
  const then =
    bd === null
      ? "When I say the files are ready, tell me to run Update Project in Branch Deck again; don't update the branch yourself."
      : `When I say the files are ready, run \`${bd.command} update --no-fetch\`${bd.powershell ? " in PowerShell" : ""} and continue as its output says. If it prints \`state: conflict\` (exit 1), run \`${bd.command} help update\` and follow it; edit only the files at \`edit\`, never the same path in the working tree.`
  return [
    `Branch Deck can't update \`${branch}\` at ${repositoryPath} onto \`${upstream}\`: my uncommitted changes are in files that ${upstream} also changes:`,
    bullets(rest === 0 ? shown : [...shown, `… and ${rest} more: \`git status\` lists them`]),
    `Other sessions work in this tree. Don't stash, reset, check out or commit these files. Ask me what to do with ${rest === 0 ? "each file" : "these files"}. ${then}`,
  ].join("\n")
}

/**
 * The likely conflicting files a prompt names. Both lists come from the backend (`update_branch.rs`), which attributes
 * commits only to its first 50 files (`MAX_ATTRIBUTED_FILES`); the rest of `files` is one line, counted as in the title
 * of the dialog. Without a limit a large conflict wrote each upstream subject once per file: hundreds of kilobytes. Each
 * limit cuts as `cutList` does: never a count of one where the item would stand
 */
const MAX_PROMPT_FILES = 20
/** The commit ids of one side of a file, then `+N more`: mine the oldest (the first to stop a replay), upstream the newest */
const MAX_FILE_COMMITS = 3
/** The upstream commits the prompt without bd describes in full, once, newest first; with bd each round names those of its files */
const MAX_DESCRIBED_UPSTREAM_COMMITS = 10
/** `MAX_UPSTREAM_COMMITS` of `update_branch.rs`: a list this long may lack the older upstream commits of a file */
const BACKEND_UPSTREAM_COMMITS = 50

interface OutlinedFile {
  path: string
  /** `bbbbbbbb, dddddddd +4 more`, empty without local commits */
  local: string
  /** `cccccccc +2 more`, newest first, empty without upstream commits */
  upstream: string
}

/** What a conflict prompt names: the first files, each with the ids of a few commits per side, and those commits */
interface ConflictOutline {
  files: OutlinedFile[]
  /** The likely conflicting files left out */
  moreFiles: number
  /** The upstream commits the files name, newest first */
  upstream: UpdateCommit[]
}

function commitIds(commits: readonly UpdateCommit[], rest: number, cut: boolean): string {
  const ids = commits.map(commit => formatShortHash(commit.id)).join(", ")
  return rest === 0 ? ids : `${ids} +${rest}${cut ? " or" : ""} more`
}

function conflictOutline(state: ConflictState): ConflictOutline {
  // the first commit that conflicts, with the files of its merge: where the rebase stops first
  const first = state.firstConflict
  const cut = first.upstreamCommits.length >= BACKEND_UPSTREAM_COMMITS
  const upstream = new Set<string>()
  // the backend reports the first files and counts all
  const { shown, more: moreFiles } = cutList(first.files, MAX_PROMPT_FILES, first.fileCount)
  const files = groupConflictFiles(shown, [first.commit], first.upstreamCommits).map(file => {
    const mine = cutList(file.local, MAX_FILE_COMMITS).shown
    const theirs = cutList([...file.upstream].reverse(), MAX_FILE_COMMITS).shown
    for (const commit of theirs) {
      upstream.add(commit.id)
    }
    return {
      path: file.path,
      local: commitIds(mine, file.local.length - mine.length, false),
      upstream: commitIds(theirs, file.upstream.length - theirs.length, cut),
    }
  })
  return {
    files,
    moreFiles,
    upstream: first.upstreamCommits.filter(commit => upstream.has(commit.id)).reverse(),
  }
}

/**
 * `First conflict: bbbbbbbb (auth) Change line 2: src/app.txt, src/config.txt`: where bd opens its first round, the files
 * of its merge, at most `MAX_PROMPT_FILES`, then the count of the rest; each round names the upstream commits of its
 * files
 */
function firstConflictLine(state: ConflictState): string {
  const first = state.firstConflict
  const commit = `${formatShortHash(first.commit.id)} ${first.commit.subject}`
  // the backend reports the first files and counts all
  const { shown, more } = cutList(first.files, MAX_PROMPT_FILES, first.fileCount)
  if (shown.length === 0) {
    return `First conflict: ${commit}.`
  }
  return `First conflict: ${commit}: ${shown.join(", ")}${more === 0 ? "" : `, and ${more} more files`}.`
}

/**
 * The upstream commits the files name, each once, newest first: at most `MAX_DESCRIBED_UPSTREAM_COMMITS` lines, then the
 * `git log` that lists the rest
 */
function upstreamCommitsSection(check: UpdateCheck, commits: readonly UpdateCommit[]): string {
  const { shown, more } = cutList(commits, MAX_DESCRIBED_UPSTREAM_COMMITS)
  const log = `\`git log --oneline ${check.base}..${check.upstreamHead} -- <path>\` lists the upstream commits of a file`
  const lines = [...shown.map(commit => upstreamCommitLine(commit, check.upstream)), ...(more === 0 ? [] : [`… and ${more} more: ${log}`])]
  return `Upstream commits of these files, newest first:\n${bullets(lines)}`
}

/**
 * The conflict prompt with bd: the agent updates with `bd update --no-fetch` (the baseline the user saw) and resolves
 * each round in the copies bd writes outside the working tree, never in the working tree itself; no `--expect-head`,
 * because peers commit meanwhile and become more rounds. The prompt holds only what bd doesn't know (the repository,
 * the command, what the user saw, the user's rules, what to report); the protocol is bd's own: `bd help update`, the
 * `hint` of each file, `session.next`, `ask_user` and `retryable`. So the prompt never restates an exit code, a marker
 * rule or an error code that bd could change, and it asks the user only where bd says `ask_user`
 */
function conflictPromptWithBd(repositoryPath: string, check: UpdateCheck, state: ConflictState, fetchTime: number | null, bd: AgentCliCommand): string {
  const { branch, upstream } = check
  const fixup = `git -C ${shellWord(repositoryPath, bd.powershell)} commit -m 'fixup! <subject>' -- <paths>`
  const fetched = fetchTime === null ? "" : ` (fetched ${new Date(fetchTime).toISOString().replace(/\.\d{3}Z$/, "Z")})`
  const heads = `HEAD (${branch}) ${formatShortHash(check.head)}, ${upstream} ${formatShortHash(check.upstreamHead)}.`
  return [
    `Update the branch \`${branch}\` of the Git repository at ${repositoryPath} onto \`${upstream}\` with Branch Deck's bd, and resolve the conflicts. bd replays my commits outside the working tree.`,
    `bd is \`${bd.command}\`${bd.powershell ? ", run in PowerShell" : ""}; below, \`bd\` stands for it. Run \`bd help update\` first and follow it: it explains each field, hint and error. Each run prints one JSON object.`,
    [`State${fetched}: ${heads}`, firstConflictLine(state)].join("\n"),
    `Steps:\n${numbered([
      "`bd update --no-fetch`. When an output or its `error` has `ask_user: true`, show me its `reasons` (or the error's `message`) with `session.blocked_paths` and `session.landed_differs`, and ask me once. A later round of a commit I decided in `landed_differs` takes that answer.",
      "While it exits 1: follow the `hint` of each file and of `session.commit`, edit only the copies at `edit`, and run `session.next`.",
      `Exit 0: build, and run the tests of \`verify.paths\`; without \`verify\`, the build is enough. On a failure, commit only the files you fixed, as \`${fixup}\`${bd.powershell ? " (double each `'` of the subject)" : ""}, and run \`bd fold\`; or run \`bd undo <undo_token>\` and tell me.`,
      "Report each edited resolution of `resolutions` and `resolved` with every line of `dropped_upstream_lines` and `dropped_commit_lines`, each drop I decided (`landed_as` with `local_only`), and `dropped` as a count per `proof`.",
    ])}`,
    `Rules:\n${bullets([
      "Other sessions work in this tree. Never run `git rebase`, `git stash`, `git checkout` or `git reset`, and never edit a conflicting file in the working tree.",
      ...UPDATE_RULES,
      "On an error: when `retryable` is true, go to step 1; else do what its `message` says, or stop and show me the error.",
    ])}`,
  ].join("\n\n")
}

/** The first local commit that conflicts (`firstConflict`): the rebase stops there first; `git log` lists the branch */
function firstConflictSection(check: UpdateCheck, commit: UpdateCommit): string {
  const log = `\`git log --reverse --oneline ${check.base}..${check.head}\` lists my commits`
  return `First conflict: my commit ${formatShortHash(commit.id)} ${commit.subject}, where the rebase stops first; ${log}.`
}

/**
 * The prompt for an external coding agent that does the update Branch Deck refused: a rebase with conflicts, or one
 * blocked by uncommitted changes on files that the upstream changed. Copied to the clipboard. With bd, a conflict is
 * resolved in bd's resolve session, outside the working tree; without, the agent rebases in a terminal, told the exact
 * state and what it must not change, and asks the user about uncommitted changes instead of stashing them. A blocked
 * update goes to the user about each file, then to bd.
 */
export function buildAgentPrompt({ repositoryPath, check, fetchTime, bd }: AgentPromptInput): string {
  const { state, branch, upstream } = check
  if (state.status === "blocked") {
    return blockedPrompt(repositoryPath, check, state, bd)
  }
  if (state.status === "conflict" && bd !== null) {
    return conflictPromptWithBd(repositoryPath, check, state, fetchTime, bd)
  }
  const sections: string[] = []
  if (state.status === "conflict") {
    const outline = conflictOutline(state)
    sections.push(
      `Rebase the branch \`${branch}\` of the Git repository at ${repositoryPath} onto \`${upstream}\` and resolve the conflicts.`,
      stateLines(check, fetchTime),
      firstConflictSection(check, state.firstConflict.commit),
    )
    if (outline.files.length > 0) {
      const files = outline.files.map(({ path, local, upstream }) => {
        const sides = [...(local === "" ? [] : [`changed by my ${local}`]), ...(upstream === "" ? [] : [`changed upstream by ${upstream}`])]
        return [path, ...sides.map(side => `  - ${side}`)].join("\n")
      })
      const more = outline.moreFiles === 0 ? [] : [`… and ${outline.moreFiles} more files: \`git status\` names them when the rebase stops`]
      sections.push(`Files of the first conflict:\n${bullets([...files, ...more])}`)
    }
    if (outline.upstream.length > 0) {
      sections.push(upstreamCommitsSection(check, outline.upstream))
    }
    sections.push(
      `Rules:\n${bullets([
        ...RULES,
        "Other sessions may work in this tree: if `git status` shows uncommitted changes, stop and ask me what to do with them. Never stash, reset, check out or commit them.",
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
  return sections.join("\n\n")
}
