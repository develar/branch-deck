import { describe, expect, it, vi } from "vite-plus/test"
import type { PullRequest, RobotLandedBranch } from "@/lib/bindings"
import { findLandedPullRequests } from "@/lib/landed-stack"
import type { LandingFacts } from "@/lib/landed-stack"
import { testRobot } from "@/lib/test-robot"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
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

/** A branch the robot landed, as the repository view lists it (`landedByRobot`) */
function landed(name: string, patch: Partial<RobotLandedBranch> = {}): RobotLandedBranch {
  return { branch: name, own: false, after: [], ...patch }
}

function facts(landedBranches: RobotLandedBranch[], branches: Branch[], pullRequests: Record<string, PullRequest>): LandingFacts {
  const byName = Object.fromEntries(branches.map(it => [it.name, it]))
  return { landed: landedBranches, branchOf: name => byName[name], pullRequestOf: name => pullRequests[name] }
}

describe("findLandedPullRequests", () => {
  const auth = branch("auth", null)
  // pushed before the last sync rebuilt it on a newer baseline: its remote has nothing it lacks
  const login = branch("login", "auth", { remoteTip: "login-old" })
  const oauth = branch("oauth", "login")
  const merged = testRobot("r", "oauth", { status: "success" })

  it("closes the pull requests of the branches a merged robot landed whole, not of its own branch", () => {
    const pullRequests = { auth: openPullRequest(5, "auth-tip"), login: openPullRequest(6, "login-old"), oauth: openPullRequest(7, "oauth-tip") }
    expect(findLandedPullRequests(merged, facts([landed("auth"), landed("login"), landed("oauth", { own: true })], [auth, login, oauth], pullRequests))).toEqual([
      { branch: "auth", number: 5 },
      { branch: "login", number: 6 },
    ])
  })

  it("leaves a pull request with a commit the branch lacks, one of a branch with work left, and one of a branch the table lacks", () => {
    const behind = branch("login", "auth", { remoteTip: "login-other", commitsBehind: 1 })
    const pullRequests = { auth: openPullRequest(5, "auth-tip"), login: openPullRequest(6, "login-other") }
    expect(findLandedPullRequests(merged, facts([landed("auth"), landed("login")], [auth, behind, oauth], pullRequests))).toEqual([{ branch: "auth", number: 5 }])
    expect(findLandedPullRequests(merged, facts([landed("auth", { after: ["auth-2"] }), landed("gone")], [auth, oauth], pullRequests))).toEqual([])
  })

  it("leaves the pull request the robot started from to Patronus", () => {
    const side = branch("side", "auth")
    const pullRequests = { auth: openPullRequest(5, "auth-tip"), side: openPullRequest(7, "side-tip") }
    const whole = testRobot("r", null, { stackRoot: "auth", sourceRevision: "stack-tip", pullRequest: { number: 7, url: "u7" } })
    expect(findLandedPullRequests(whole, facts([landed("auth"), landed("side")], [auth, side], pullRequests))).toEqual([{ branch: "auth", number: 5 }])
  })
})
