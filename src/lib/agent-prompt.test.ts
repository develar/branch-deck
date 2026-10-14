// @spec spec/repository/update-project.spec.md
import { describe, expect, it } from "vitest"
import { buildAgentPrompt } from "@/lib/agent-prompt"
import type { UpdateCheck, UpdateState } from "@/lib/bindings"

const H = "1111111111111111111111111111111111111111"
const U = "2222222222222222222222222222222222222222"
const B = "3333333333333333333333333333333333333333"

function check(state: UpdateState): UpdateCheck {
  return { branch: "master", upstream: "origin/master", head: H, upstreamHead: U, base: B, state, fetchedAt: null }
}

const conflict = check({
  status: "conflict",
  files: ["src/app.txt"],
  commits: [
    { id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", subject: "(auth) Add login" },
    { id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", subject: "(auth) Change line 2" },
  ],
})

describe("buildAgentPrompt", () => {
  it("names the repository, the branch, the upstream and the exact ids", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: Date.UTC(2026, 9, 14, 7, 30) })
    expect(prompt).toContain("Rebase the branch `master` of the Git repository at /work/repo onto `origin/master`")
    expect(prompt).toContain(`- HEAD (master): ${H}`)
    expect(prompt).toContain(`- origin/master (fetched 2026-10-14T07:30:00.000Z): ${U}`)
    expect(prompt).toContain(`- Merge base: ${B}`)
  })

  it("lists the local commits oldest first and the likely conflicting files", () => {
    const prompt = buildAgentPrompt({ repositoryPath: "/work/repo", check: conflict, fetchTime: null })
    expect(prompt).toContain("My local commits, oldest first:\n- aaaaaaaa (auth) Add login\n- bbbbbbbb (auth) Change line 2")
    expect(prompt).toContain("Files that likely conflict:\n- src/app.txt")
    expect(prompt).not.toContain("fetched")
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
    const prompt = buildAgentPrompt({ repositoryPath: "/r", check: check({ status: "conflict", files: [], commits: [] }), fetchTime: null })
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
