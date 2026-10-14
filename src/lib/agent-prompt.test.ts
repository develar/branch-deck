// @spec spec/repository/update-project.spec.md
import { describe, expect, it } from "vitest"
import { buildAgentPrompt } from "@/lib/agent-prompt"
import type { UpdateCheck, UpdateCommit, UpdateState } from "@/lib/bindings"

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

const conflict = check({
  status: "conflict",
  files: ["src/app.txt", "src/config.txt"],
  commits: [
    commit("a", "(auth) Add login", "Me"),
    commit("b", "(auth) Change line 2", "Me", ["src/app.txt"]),
  ],
  // landed on 2023-11-16
  upstreamCommits: [commit("c", "Change line 2 upstream", "Dana Lee", ["src/app.txt", "src/config.txt"], 1_700_100_000)],
})

describe("buildAgentPrompt", () => {
  it("names the repository, the branch, the upstream and the exact ids", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: Date.UTC(2026, 9, 14, 7, 30) })
    expect(prompt).toContain("Rebase the branch `master` of the Git repository at /work/repo onto `origin/master`")
    expect(prompt).toContain(`- HEAD (master): ${H}`)
    expect(prompt).toContain(`- origin/master (fetched 2026-10-14T07:30:00.000Z): ${U}`)
    expect(prompt).toContain(`- Merge base: ${B}`)
  })

  it("lists the local commits oldest first", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: null })
    expect(prompt).toContain("My local commits, oldest first:\n- aaaaaaaa (auth) Add login\n- bbbbbbbb (auth) Change line 2")
    expect(prompt).not.toContain("fetched")
  })

  it("names the commits on both sides of each likely conflicting file", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: null })
    // an upstream commit with its author, when it was written, and when it landed (the committer time)
    const upstream = "cccccccc Change line 2 upstream (Dana Lee, authored 2023-11-14, in origin/master since 2023-11-16)"
    expect(prompt).toContain([
      "Files that likely conflict:",
      "- src/app.txt",
      "  - changed by my bbbbbbbb",
      `  - changed upstream by ${upstream}`,
      "- src/config.txt",
      `  - changed upstream by ${upstream}`,
    ].join("\n"))
  })

  it("states the rules and the steps", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: null })
    expect(prompt).toContain("Keep every commit message unchanged, including the `(prefix)`")
    expect(prompt).toContain("Don't squash, reorder, split or drop commits.")
    expect(prompt).toContain("Don't push.")
    expect(prompt).toContain("`git stash push --include-untracked` first and `git stash pop` at the end")
    expect(prompt).toContain("1. `git rebase origin/master`.")
    expect(prompt).toContain("keep the upstream change and reapply the intent of my commit")
    expect(prompt).toContain("Build and run the tests")
    expect(prompt).toContain("`git rebase --continue`")
    expect(prompt).toContain("Report each resolution")
  })

  it("leaves out the file list when the conflicting files are unknown", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/r", check: check({ status: "conflict", files: [], commits: [], upstreamCommits: [] }), fetchTime: null })
    expect(prompt).not.toContain("Files that likely conflict")
  })

  it("asks to stash, update and restore uncommitted changes that block the update", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: check({ status: "blocked", paths: ["src/app.txt", "build.out"] }), fetchTime: null })
    expect(prompt).toContain("Update the branch `master` of the Git repository at /work/repo to `origin/master`.")
    expect(prompt).toContain("My uncommitted changes that overlap the upstream:\n- src/app.txt\n- build.out")
    expect(prompt).toContain("1. `git stash push --include-untracked`.")
    expect(prompt).toContain("3. `git stash pop`.")
    expect(prompt).toContain("Keep my uncommitted changes uncommitted")
    expect(prompt).toContain("Don't push.")
  })
})
