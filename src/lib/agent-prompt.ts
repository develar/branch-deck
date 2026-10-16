import type { AgentCliCommand, UpdateCheck, UpdateCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { bullets, numbered, RULES, shellWord } from "@/lib/prompt-text"
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
  return bullets([
    `HEAD (${check.branch}): ${check.head}`,
    `${check.upstream}${fetched}: ${check.upstreamHead}`,
    `Merge base: ${check.base}`,
  ])
}

/**
 * The blocked prompt: other sessions may work in the same tree, so the agent must not stash, reset, check out or
 * commit the files; it asks the user about each. Then bd updates, which writes only the files the update changes, or,
 * without a bd command, the user updates in Branch Deck. Never a stash: it would take the files of the other sessions
 */
function blockedPrompt(repositoryPath: string, check: UpdateCheck, paths: readonly string[], bd: AgentCliCommand | null): string {
  const { branch, upstream } = check
  const shown = paths.slice(0, MAX_BLOCKED_PATHS)
  const rest = paths.length - shown.length
  const then = bd === null
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
 * of the dialog. Without a limit a large conflict wrote each upstream subject once per file: hundreds of kilobytes
 */
const MAX_PROMPT_FILES = 20
/** The commit ids of one side of a file, then `+N more`: mine the oldest (the first to stop a replay), upstream the newest */
const MAX_FILE_COMMITS = 3
/** The upstream commits described in full, once, newest first; with bd each round lists those of its files */
const MAX_DESCRIBED_UPSTREAM_COMMITS = 10
/** The local commits the prompt without bd lists; more are only the ones its files name */
const MAX_LISTED_COMMITS = 30
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
  /** The local commits the files name, oldest first */
  local: UpdateCommit[]
  /** The upstream commits the files name, newest first */
  upstream: UpdateCommit[]
}

function commitIds(commits: readonly UpdateCommit[], rest: number, cut: boolean): string {
  const ids = commits.map(commit => formatShortHash(commit.id)).join(", ")
  return rest === 0 ? ids : `${ids} +${rest}${cut ? " or" : ""} more`
}

function conflictOutline(state: ConflictState): ConflictOutline {
  const cut = state.upstreamCommits.length >= BACKEND_UPSTREAM_COMMITS
  const local = new Set<string>()
  const upstream = new Set<string>()
  const files = groupConflictFiles(state.files.slice(0, MAX_PROMPT_FILES), state.commits, state.upstreamCommits).map((file) => {
    const mine = file.local.slice(0, MAX_FILE_COMMITS)
    const theirs = file.upstream.slice(-MAX_FILE_COMMITS).reverse()
    for (const commit of mine) {
      local.add(commit.id)
    }
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
    moreFiles: state.files.length - files.length,
    local: state.commits.filter(commit => local.has(commit.id)),
    upstream: state.upstreamCommits.filter(commit => upstream.has(commit.id)).reverse(),
  }
}

/** `Likely conflict: src/app.txt (mine: bbbbbbbb; upstream: cccccccc)` per file, then the count of the rest */
function likelyConflictLines({ files, moreFiles }: ConflictOutline): string[] {
  const lines = files.map(({ path, local, upstream }) => {
    const sides = [
      ...(local === "" ? [] : [`mine: ${local}`]),
      ...(upstream === "" ? [] : [`upstream: ${upstream}`]),
    ]
    return `Likely conflict: ${sides.length === 0 ? path : `${path} (${sides.join("; ")})`}`
  })
  return moreFiles === 0 ? lines : [...lines, `… and ${moreFiles} more files: bd names the files of each round`]
}

/** The upstream commits the files name, each once, newest first: at most `MAX_DESCRIBED_UPSTREAM_COMMITS` lines */
function upstreamCommitsSection(commits: readonly UpdateCommit[], line: (commit: UpdateCommit) => string, rest: string): string {
  const shown = commits.slice(0, MAX_DESCRIBED_UPSTREAM_COMMITS)
  const more = commits.length - shown.length
  return `Upstream commits of these files, newest first:\n${bullets([...shown.map(line), ...(more === 0 ? [] : [`… and ${more} more: ${rest}`])])}`
}

/**
 * The conflict prompt with bd: the agent updates with `bd update --no-fetch` (the baseline the user saw) and resolves
 * each round in the copies bd writes outside the working tree, never in the working tree itself; no `--expect-head`,
 * because peers commit meanwhile and become more rounds
 */
function conflictPromptWithBd(repositoryPath: string, check: UpdateCheck, state: ConflictState, fetchTime: number | null, bd: AgentCliCommand): string {
  const { branch, upstream } = check
  const fixup = `git -C ${shellWord(repositoryPath, bd.powershell)} commit -m 'fixup! <subject>' -- <paths>`
  const fetched = fetchTime === null ? "" : ` (fetched ${new Date(fetchTime).toISOString()})`
  const outline = conflictOutline(state)
  return [
    `Update the branch \`${branch}\` of the Git repository at ${repositoryPath} onto \`${upstream}\` with Branch Deck's bd, and resolve the conflicts. bd replays my commits outside the working tree.`,
    `bd is \`${bd.command}\`${bd.powershell ? ", run in PowerShell" : ""}; below, \`bd\` stands for it. Each run prints one JSON object: act on \`state\`, \`error.code\` and the exit code. \`bd help update\` explains each field.`,
    `State${fetched}:\n${bullets([
      `HEAD (${branch}): ${check.head}`,
      `${upstream}: ${check.upstreamHead}`,
      ...likelyConflictLines(outline),
    ])}`,
    ...(outline.upstream.length === 0 ? [] : [upstreamCommitsSection(outline.upstream, commit => `${formatShortHash(commit.id)} ${commit.subject}, ${commit.author}`, "each round lists the upstream commits of its files")]),
    `Steps:\n${numbered([
      "`bd update --no-fetch`. If `session.blocked_paths` is not empty, ask me about those files first.",
      `Exit 1: for each entry of \`session.files\`, edit the file at \`edit\`. Keep the change of \`onto\` and reapply the intent of \`session.commit\` (its \`patch\`). Remove every conflict marker.${bd.powershell ? " Write each copy with `[IO.File]::WriteAllText('<edit>', $text)`: `Set-Content` and `Out-File` add a byte order mark or write UTF-16, and change the line ends." : ""} For \`kind\` modify_delete, add \`--take\` or \`--delete\` to the command of step 3, as its \`hint\` says. A change the commit needs outside the conflict blocks waits for a \`fixup!\` commit after step 4.`,
      "`bd update --continue --round <session.round>`. Exit 1 is the next commit: go to step 2 (`state: pending`: run `session.next`). Exit 2: `error.message` says what to fix (the copies of `error.paths`; `round_moved`: go to step 1; `empty_resolution`: ask me).",
      `Exit 0: build and run the tests of the code you touched. On a failure, commit only the files you fixed, as \`${fixup}\`${bd.powershell ? " (double each `'` of the subject)" : ""}, and run \`bd fold\`; or run \`bd undo <undo_token>\` and tell me.`,
      "Report each entry of `resolutions`: the file, the commit, what you kept from each side, and every line of `dropped_upstream_lines` and `dropped_commit_lines`.",
    ])}`,
    `Rules:\n${bullets([
      "Other sessions work in this tree. Never run `git rebase`, `git stash`, `git checkout` or `git reset`.",
      "Edit only the files at `edit`, never the same path in the working tree.",
      ...RULES,
      "Exit 3: run the same command again. Exit 4: stop and show me the error.",
    ])}`,
  ].join("\n\n")
}

/**
 * The local commits, oldest first: all of them up to `MAX_LISTED_COMMITS`, else those the files name (all, without
 * known files), cut at that many, with the count and the command that lists the branch
 */
function localCommitsSection(check: UpdateCheck, commits: readonly UpdateCommit[], named: readonly UpdateCommit[]): string {
  const line = (commit: UpdateCommit) => `${formatShortHash(commit.id)} ${commit.subject}`
  if (commits.length <= MAX_LISTED_COMMITS) {
    return `My local commits, oldest first:\n${bullets(commits.map(line))}`
  }
  const listed = named.length === 0 ? commits : named
  const shown = listed.slice(0, MAX_LISTED_COMMITS)
  const more = listed.length - shown.length
  const title = named.length === 0 ? "My local commits" : "My local commits that change these files"
  const all = `${commits.length} in all; \`git log --reverse --oneline ${check.base}..${check.head}\` lists the branch`
  return `${title}, oldest first (${all}):\n${bullets([...shown.map(line), ...(more === 0 ? [] : [`… and ${more} more`])])}`
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
    return blockedPrompt(repositoryPath, check, state.paths, bd)
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
      localCommitsSection(check, state.commits, outline.local),
    )
    if (outline.files.length > 0) {
      const files = outline.files.map(({ path, local, upstream }) => {
        const sides = [
          ...(local === "" ? [] : [`changed by my ${local}`]),
          ...(upstream === "" ? [] : [`changed upstream by ${upstream}`]),
        ]
        return [path, ...sides.map(side => `  - ${side}`)].join("\n")
      })
      const more = outline.moreFiles === 0 ? [] : [`… and ${outline.moreFiles} more files: \`git status\` names them when the rebase stops`]
      sections.push(`Files that likely conflict:\n${bullets([...files, ...more])}`)
    }
    if (outline.upstream.length > 0) {
      const log = `\`git log --oneline ${check.base}..${check.upstreamHead} -- <path>\` lists the upstream commits of a file`
      sections.push(upstreamCommitsSection(outline.upstream, commit => upstreamCommitLine(commit, check.upstream), log))
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
