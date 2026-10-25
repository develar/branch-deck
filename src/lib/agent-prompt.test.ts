// @spec spec/repository/update-project.spec.md
import { describe, expect, it } from "vite-plus/test"
import { buildAgentPrompt } from "@/lib/agent-prompt"
import type { UpdateCheck, UpdateCommit, UpdateState } from "@/lib/bindings"
import { conflictState } from "@/test-support/update-conflict"

const H = "1111111111111111111111111111111111111111"
const U = "2222222222222222222222222222222222222222"
const B = "3333333333333333333333333333333333333333"

function check(state: UpdateState): UpdateCheck {
  return { branch: "master", upstream: "origin/master", head: H, upstreamHead: U, base: B, state, fetchedAt: null }
}

/** Authored 2023-11-14 (UTC); committed then too, unless `committerTime` says otherwise */
function commit(id: string, subject: string, author: string, files: string[] = [], committerTime = 1_700_000_000): UpdateCommit {
  return { id: id.repeat(40), subject, author, time: 1_700_000_000, committer: author, committerTime, files }
}

// the first commit that conflicts; the upstream commit landed on 2023-11-16
const conflict = check(
  conflictState(["src/app.txt", "src/config.txt"], 2, commit("b", "(auth) Change line 2", "Me"), [
    commit("c", "Change line 2 upstream", "Dana Lee", ["src/app.txt"], 1_700_100_000),
  ]),
)

describe("buildAgentPrompt", () => {
  it("names the repository, the branch, the upstream and the exact ids", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: Date.UTC(2026, 9, 14, 7, 30), bd: null })
    expect(prompt).toContain("Rebase the branch `master` of the Git repository at /work/repo onto `origin/master`")
    expect(prompt).toContain(`- HEAD (master): ${H}`)
    expect(prompt).toContain(`- origin/master (fetched 2026-10-14T07:30:00.000Z): ${U}`)
    expect(prompt).toContain(`- Merge base: ${B}`)
  })

  it("names the first commit that conflicts and how to list the others", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: null, bd: null })
    expect(prompt).toContain(`The rebase stops first at my commit bbbbbbbb (auth) Change line 2; \`git log --reverse --oneline ${B}..${H}\` lists my commits.`)
    expect(prompt).not.toContain("fetched")
  })

  it("names the commits on both sides of each likely conflicting file", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: null, bd: null })
    // an upstream commit with its author, when it was written, and when it landed (the committer time)
    const upstream = "cccccccc Change line 2 upstream (Dana Lee, authored 2023-11-14, in origin/master since 2023-11-16)"
    expect(prompt).toContain(
      [
        "Files that likely conflict:",
        "- src/app.txt",
        "  - changed by my bbbbbbbb",
        "  - changed upstream by cccccccc",
        "- src/config.txt",
        "  - changed by my bbbbbbbb",
        "",
        "Upstream commits of these files, newest first:",
        `- ${upstream}`,
      ].join("\n"),
    )
    // each commit described once
    expect(prompt.split("Change line 2 upstream").length - 1).toBe(1)
  })

  it("states the rules and the steps", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: null, bd: null })
    expect(prompt).toContain("Keep every commit message unchanged, including the `(prefix)`")
    expect(prompt).toContain("Don't squash, reorder, split or drop commits.")
    expect(prompt).toContain("Don't push.")
    expect(prompt).toContain("if `git status` shows uncommitted changes, stop and ask me what to do with them")
    expect(prompt).not.toContain("git stash")
    expect(prompt).toContain("1. `git rebase origin/master`.")
    expect(prompt).toContain("keep the upstream change and reapply the intent of my commit")
    expect(prompt).toContain("Build and run the tests")
    expect(prompt).toContain("`git rebase --continue`")
    expect(prompt).toContain("Report each resolution")
  })

  it("leaves out the file list when the conflicting files are unknown", () => {
    const prompt = buildAgentPrompt({
      repositoryPath: "/r",
      check: check(conflictState([], 0)),
      fetchTime: null,
      bd: null,
    })
    expect(prompt).not.toContain("Files that likely conflict")
  })

  const blocked = check({ status: "blocked", paths: ["src/app.txt", "build.out"], pathCount: 2 })
  const blockedLines = [
    "Branch Deck can't update `master` at /work/my repo onto `origin/master`: my uncommitted changes are in files that origin/master also changes:",
    "- src/app.txt",
    "- build.out",
  ]
  const askFirst = "Other sessions work in this tree. Don't stash, reset, check out or commit these files. Ask me what to do with each file."

  it("with bd, sends a blocked update to the user and then to bd, never to a stash", () => {
    const bd = { command: "/Users/me/.local/bin/bd -C '/work/my repo'", powershell: false }
    const prompt = buildAgentPrompt({ repositoryPath: "/work/my repo", check: blocked, fetchTime: null, bd })
    expect(prompt).toBe(
      [
        ...blockedLines,
        `${askFirst} When I say the files are ready, run \`/Users/me/.local/bin/bd -C '/work/my repo' update --no-fetch\` and continue as its output says. If it prints \`state: conflict\` (exit 1), run \`/Users/me/.local/bin/bd -C '/work/my repo' help update\` and follow it; edit only the files at \`edit\`, never the same path in the working tree.`,
      ].join("\n"),
    )
    expect(prompt).not.toContain("git stash")
  })

  it("names PowerShell for the command of Windows", () => {
    const bd = { command: "& 'C:\\Program Files\\Branch Deck\\bd.exe' -C 'C:\\work\\my repo'", powershell: true }
    const prompt = buildAgentPrompt({ repositoryPath: "/work/my repo", check: blocked, fetchTime: null, bd })
    expect(prompt).toContain(`run \`${bd.command} update --no-fetch\` in PowerShell and continue as its output says. If it prints`)
  })

  it("without bd, sends a blocked update to the user and back to Branch Deck, never to a stash", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/my repo", check: blocked, fetchTime: null, bd: null })
    expect(prompt).toBe(
      [...blockedLines, `${askFirst} When I say the files are ready, tell me to run Update Project in Branch Deck again; don't update the branch yourself.`].join("\n"),
    )
    expect(prompt).not.toContain("git ")
  })

  const bd = { command: "'/Applications/Branch Deck.app/Contents/MacOS/branch-deck' cli -C /work/repo", powershell: false }

  it("with bd, resolves a conflict in bd's session, outside the working tree", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: Date.UTC(2026, 9, 14, 7, 30), bd })
    expect(prompt).toBe(
      [
        "Update the branch `master` of the Git repository at /work/repo onto `origin/master` with Branch Deck's bd, and resolve the conflicts. bd replays my commits outside the working tree.",
        "",
        `bd is \`${bd.command}\`; below, \`bd\` stands for it. Run \`bd help update\` first and follow it: it explains each field and error. Each run prints one JSON object.`,
        "",
        "State (fetched 2026-10-14T07:30:00Z): HEAD (master) 11111111, origin/master 22222222.",
        "- Likely conflict: src/app.txt (mine: bbbbbbbb; upstream: cccccccc)",
        "- Likely conflict: src/config.txt (mine: bbbbbbbb)",
        "",
        "Steps:",
        "1. `bd update --no-fetch`. If `session.blocked_paths` is not empty, ask me about those files first.",
        "2. While it exits 1: resolve each entry of `session.files` as its `hint` says (and `session.commit.hint`, if any), editing only the copy at `edit` and only the lines between the markers, then run `session.next` with the `--take` or `--delete` the hint names. A change the commit needs elsewhere waits for the fixup of step 3.",
        "3. Exit 0: build and run the tests of the code you touched. On a failure, commit only the files you fixed, as `git -C /work/repo commit -m 'fixup! <subject>' -- <paths>`, and run `bd fold`; or run `bd undo <undo_token>` and tell me.",
        "4. Report each entry of `resolutions` and of `resolved`: the file, the commit, what you kept from each side, and every line of `dropped_upstream_lines` and `dropped_commit_lines`.",
        "",
        "Rules:",
        "- Other sessions work in this tree. Never run `git rebase`, `git stash`, `git checkout` or `git reset`, and never edit a conflicting file in the working tree.",
        "- Keep every commit message unchanged, including the `(prefix)` at the start of a subject: Branch Deck groups commits by it.",
        "- Don't squash, reorder, split or drop commits.",
        "- Don't push.",
        "- On an error: `retryable` true: go to step 1. Exit 2 with `paths`: fix those copies and run the command again. `uncommitted_overlap` or `empty_resolution`: ask me. Anything else: stop and show me the error.",
      ].join("\n"),
    )
    expect(prompt).not.toContain("--expect-head")
    // the protocol is bd's own (`bd help update`, `hint`, `session.next`, `retryable`): no exit code or error code that
    // bd could change under the prompt, as `round_moved` moved from exit 2 to 3
    expect(prompt).not.toMatch(/round_moved|Exit [345]/)
    // a round names the upstream commits of its files
    expect(prompt).not.toContain("Upstream commits of these files")
    // the command once, not per step
    expect(prompt.split(bd.command).length - 1).toBe(1)
  })

  it("names PowerShell for a conflict on Windows", () => {
    const powershell = { command: "& 'C:\\Program Files\\Branch Deck\\bd.exe' -C 'C:\\work\\repo'", powershell: true }
    const prompt = buildAgentPrompt({ repositoryPath: "C:\\work\\repo", check: conflict, fetchTime: null, bd: powershell })
    expect(prompt).toContain(`bd is \`${powershell.command}\`, run in PowerShell; below, \`bd\` stands for it.`)
    // a copy written so that bd reads it, and a fixup commit in the repository, its quote doubled
    expect(prompt).toContain("Write each copy with `[IO.File]::WriteAllText('<edit>', $text)`")
    const quoted = buildAgentPrompt({ repositoryPath: "C:\\work\\it's repo", check: conflict, fetchTime: null, bd: powershell })
    expect(quoted).toContain("`git -C 'C:\\work\\it''s repo' commit -m 'fixup! <subject>' -- <paths>` (double each `'` of the subject)")
  })

  describe("a large conflict", () => {
    /** The id `<side><7 hex digits of n>…`, so that its short hash tells the commits apart */
    const id = (side: number, n: number) => `${side}${n.toString(16).padStart(7, "0")}`.padEnd(40, String(side))
    // the backend reports the first 50 files with their commits and counts all
    const files = Array.from({ length: 50 }, (_, i) => `platform/core-impl/src/com/example/very/long/package/name/Component${i}.kt`)
    // the first commit that conflicts
    const first = { ...commit("0", "(auth) Local change number 0 with a long enough subject", "Me"), id: id(1, 0) }
    // the backend's cut: the newest 50 of the upstream commits
    const upstreamCommits = Array.from({ length: 50 }, (_, i) => ({ ...commit("0", `Upstream change number ${i} with a long enough subject`, `Author ${i}`, files), id: id(2, i) }))
    const large = check(conflictState(files, 300, first, upstreamCommits))

    /** `described`: the prompt describes the upstream commits the files name, each once (without bd) */
    function expectShort(prompt: string, lines: number, described: boolean) {
      // 300 files with 50 upstream commits each were hundreds of kilobytes
      expect(prompt.length).toBeLessThan(10_000)
      expect(prompt.split("\n").filter(line => line.includes("Component")).length).toBe(lines)
      for (let i = 0; i < 50; i++) {
        expect(prompt.split(`Upstream change number ${i} `).length - 1).toBe(described && i >= 47 ? 1 : 0)
      }
    }

    it("with bd, names the first files and a few commits of each by id", () => {
      const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: large, fetchTime: null, bd })
      expectShort(prompt, 20, false)
      expect(prompt).toContain(`- Likely conflict: ${files[0]} (mine: 10000000; upstream: 20000031, 20000030, 2000002f +47 or more)`)
      expect(prompt).toContain("- … and 280 more files: bd names the files of each round")
      expect(prompt).not.toContain("… and 0 more")
    })

    it("without bd, names the first conflict and the commits of its files", () => {
      const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: large, fetchTime: null, bd: null })
      expectShort(prompt, 20, true)
      expect(prompt).toContain("The rebase stops first at my commit 10000000 (auth) Local change number 0 with a long enough subject; ")
      expect(prompt).toContain("  - changed by my 10000000\n  - changed upstream by 20000031, 20000030, 2000002f +47 or more")
      expect(prompt).toContain("- … and 280 more files: `git status` names them when the rebase stops")
      expect(prompt).toContain("- 20000031 Upstream change number 49 with a long enough subject (Author 49, authored 2023-11-14, in origin/master since 2023-11-14)")
    })

    it("counts the rest exactly when the backend did not cut the upstream list", () => {
      const prompt = buildAgentPrompt({
        repositoryPath: "/work/repo",
        check: check(conflictState(files, 300, first, upstreamCommits.slice(0, 49))),
        fetchTime: null,
        bd,
      })
      expect(prompt).toContain("; upstream: 20000030, 2000002f, 2000002e +46 more)")
    })

    it("names the first conflict without known files", () => {
      const prompt = buildAgentPrompt({
        repositoryPath: "/work/repo",
        check: check(conflictState([], 0, first)),
        fetchTime: null,
        bd: null,
      })
      expect(prompt).toContain("The rebase stops first at my commit 10000000 ")
      expect(prompt).not.toContain("Upstream commits of these files")
    })

    it("never counts one where the item would stand: a list one longer than its limit is given whole", () => {
      // 21 files (the limit is 20), 4 upstream commits of each (3)
      const wide = check(conflictState(files.slice(0, 21), 21, first, upstreamCommits.slice(0, 4)))
      const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: wide, fetchTime: null, bd })
      expect(prompt.split("\n").filter(line => line.startsWith("- Likely conflict:")).length).toBe(21)
      expect(prompt).toContain(`- Likely conflict: ${files[20]} (mine: 10000000; upstream: 20000003, 20000002, 20000001, 20000000)`)
      expect(prompt).not.toMatch(/\b1 more/)
      const withoutBd = buildAgentPrompt({ repositoryPath: "/work/repo", check: wide, fetchTime: null, bd: null })
      for (let i = 0; i < 4; i++) {
        expect(withoutBd).toContain(`Upstream change number ${i} `)
      }
      expect(withoutBd).not.toMatch(/\b1 more/)
      // 51 blocking paths (50)
      const blocked = buildAgentPrompt({
        repositoryPath: "/work/repo",
        check: check({ status: "blocked", paths: [...files, `${files[0]}.bak`], pathCount: 51 }),
        fetchTime: null,
        bd: null,
      })
      expect(blocked.split("\n").filter(line => line.includes("Component")).length).toBe(51)
      expect(blocked).not.toContain("more: `git status` lists them")
    })

    it("caps the paths of a blocked update", () => {
      const prompt = buildAgentPrompt({
        repositoryPath: "/work/repo",
        check: check({ status: "blocked", paths: [...files, ...files.map(path => `${path}.bak`)], pathCount: 300 }),
        fetchTime: null,
        bd: null,
      })
      expect(prompt.split("\n").filter(line => line.includes("Component")).length).toBe(50)
      expect(prompt).toContain(`- ${files[49]}\n- … and 250 more: \`git status\` lists them\n`)
      expect(prompt).toContain("Ask me what to do with these files.")
    })
  })
})
