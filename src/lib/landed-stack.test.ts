import { describe, expect, it, vi } from "vite-plus/test"
import type { PullRequest } from "@/lib/bindings"
import { findLandedPullRequests } from "@/lib/landed-stack"
import type { LandingFacts } from "@/lib/landed-stack"
import { landingKey, landingRequestOf } from "@/lib/patronus"
import { testLanding, testRobot } from "@/lib/test-robot"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

function branch(name: string, parent: string | null, remote: Partial<RemoteStatus> | null = null): Branch {
  return testBranch(name, parent, {
    commits: [{ hash: `${name}-tip` } as Branch["commits"][number]],
    remoteStatus: remote === null ? null : testRemoteStatus(remote),
  })
}

function openPullRequest(number: number, headRefOid: string): PullRequest {
  return { number, state: "open", headRefOid } as PullRequest
}

function facts(branches: Branch[], pullRequests: Record<string, PullRequest>, stacks: Record<string, StackBranch> = {}): LandingFacts {
  const byName = Object.fromEntries(branches.map(it => [it.name, it]))
  return { branchOf: name => byName[name], pullRequestOf: name => pullRequests[name], stackOf: root => stacks[root] }
}

describe("findLandedPullRequests", () => {
  const auth = branch("auth", null)
  // pushed before the last sync rebuilt it on a newer baseline: its remote has nothing it lacks
  const login = branch("login", "auth", { remoteTip: "login-old" })
  const oauth = branch("oauth", "login")

  it("closes the pull requests of the ancestors a merged piece landed, by content", () => {
    const pullRequests = { auth: openPullRequest(5, "auth-tip"), login: openPullRequest(6, "login-old") }
    const merged = testRobot("r", "oauth", { status: "success" })
    expect(findLandedPullRequests(merged, facts([auth, login, oauth], pullRequests))).toEqual([
      { branch: "auth", number: 5 },
      { branch: "login", number: 6 },
    ])
    // a commit on oauth since the robot took its tip: what oauth is built on is as the robot took it
    const grown = testBranch("oauth", "login", { commits: [{ hash: "oauth-new" }, { hash: "oauth-tip" }] as unknown as Branch["commits"] })
    expect(findLandedPullRequests(merged, facts([auth, login, grown], pullRequests))).toHaveLength(2)
  })

  it("leaves a pull request with a commit the branch lacks, and a robot of another state of the branch", () => {
    const behind = branch("login", "auth", { remoteTip: "login-other", commitsBehind: 1 })
    const pullRequests = { auth: openPullRequest(5, "auth-tip"), login: openPullRequest(6, "login-other") }
    expect(findLandedPullRequests(testRobot("r", "oauth"), facts([auth, behind, oauth], pullRequests))).toEqual([{ branch: "auth", number: 5 }])
    // the robot took a commit the branch moved past: what it landed is not the branch as it is
    expect(findLandedPullRequests(testRobot("r", "oauth", { sourceRevision: "older" }), facts([auth, behind, oauth], pullRequests))).toEqual([])
    expect(findLandedPullRequests(testRobot("r", "oauth", { pushMode: "dryRun" }), facts([auth, behind, oauth], pullRequests))).toEqual([])
  })

  it("decides by content after a change below the robot's commit: closes the pull requests of the branches it landed whole, not of one with work left", () => {
    // a new commit of auth rebuilt login and oauth: no hash is the robot's, their remote branches have the older copies
    const grownAuth = testBranch("auth", null, {
      commits: [{ hash: "auth-new" }, { hash: "auth-copy" }] as unknown as Branch["commits"],
      remoteStatus: testRemoteStatus({ remoteTip: "auth-old", commitsAhead: 1 }),
    })
    const rebuilt = branch("login", "auth", { remoteTip: "login-old", commitsRestacked: 1, fastForward: false })
    const pullRequests = { auth: openPullRequest(5, "auth-old"), login: openPullRequest(6, "login-old") }
    const merged = testRobot("r", "oauth", { status: "success", sourceRevision: "oauth-old" })
    const source = facts([grownAuth, rebuilt, oauth], pullRequests)
    // the hashes don't tell, and no answer of the content yet: nothing closes
    expect(findLandedPullRequests(merged, source)).toEqual([])
    const request = landingRequestOf(merged, source)!
    const landing = testLanding("r", { auth: ["auth-2"], login: [], oauth: [] })
    // auth has a commit left, which its pull request still shows; login landed whole, its remote holds nothing it lacks
    expect(findLandedPullRequests(merged, { ...source, landings: { [landingKey(request, source)]: landing } })).toEqual([{ branch: "login", number: 6 }])
  })

  it("closes the pull requests of every branch of the stack the robot of the whole stack landed, not the one it started from", () => {
    const side = branch("side", "auth")
    const stack = { root: "auth", members: ["auth", "login", "side"], tip: "stack-tip", remoteStatus: null } as unknown as StackBranch
    const pullRequests = { auth: openPullRequest(5, "auth-tip"), login: openPullRequest(6, "login-old"), side: openPullRequest(7, "side-tip") }
    const whole = testRobot("r", null, { stackRoot: "auth", sourceRevision: "stack-tip", pullRequest: { number: 7, url: "u7" } })
    expect(findLandedPullRequests(whole, facts([auth, login, side], pullRequests, { auth: stack }))).toEqual([
      { branch: "auth", number: 5 },
      { branch: "login", number: 6 },
    ])
    // a stack branch kept for its remote branch lands nothing of the branches the table has now
    expect(findLandedPullRequests(whole, facts([auth, login, side], pullRequests, { auth: { ...stack, members: [] } }))).toEqual([])
  })
})
