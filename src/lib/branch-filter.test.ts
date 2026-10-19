import { describe, expect, it } from "vite-plus/test"
import type { BranchFilter } from "@/lib/branch-filter"
import { chipCount, collectChipMatches, filterBranches, formatNoMatch, isFilterActive, keepShown, matchesBranchText, movesByUrgency, NO_FILTER } from "@/lib/branch-filter"
import type { LandingNodeKind } from "@/lib/landing-node"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

function commit(name: string, subject: string): SyncedCommit {
  return { originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: subject, message: subject, author: "Ann", authorTime: 0, committerTime: 0 }
}

/** A branch at the commit `${name}-tip` with one commit of `subject`, not pushed */
function branch(name: string, overrides: Partial<Branch> = {}, subject = `Work on ${name}`): Branch {
  return testBranch(name, null, { commits: [commit(name, subject)], remoteStatus: testRemoteStatus({ exists: false }), ...overrides })
}

/** The node of each row by name, as `getLandingNodeNow` gives it; none for a branch not listed */
function nodes(kinds: Record<string, LandingNodeKind>): (name: string) => LandingNodeKind | null {
  return name => kinds[name] ?? null
}

describe("branch filter", () => {
  it("matches every word in the name, the issue key, the summary, the description title or a commit subject, ignoring case", () => {
    const auth = branch("feature-auth", { issueKey: "IJPL-12", summary: "Add the login form", descriptionTitle: "Sign in with OAuth" }, "Hash the passwords")
    expect(matchesBranchText(auth, "AUTH")).toBe(true)
    expect(matchesBranchText(auth, "ijpl-12")).toBe(true)
    expect(matchesBranchText(auth, "login")).toBe(true)
    expect(matchesBranchText(auth, "oauth")).toBe(true)
    expect(matchesBranchText(auth, "passwords")).toBe(true)
    // the words may be in different texts, but all must be there
    expect(matchesBranchText(auth, "  feature   passwords ")).toBe(true)
    expect(matchesBranchText(auth, "feature docs")).toBe(false)
    expect(matchesBranchText(auth, "  ")).toBe(true)
  })

  it("puts a branch into the chip of the node of its row, the filled nodes into Your Move", () => {
    const kinds: Record<string, LandingNodeKind> = {
      conflict: "needsFix",
      review: "yourStep",
      approved: "ready",
      robot: "landing",
      parent: "waiting",
      blocked: "waiting",
      merged: "inMaster",
    }
    const names = [...Object.keys(kinds), "syncing"]
    const matches = collectChipMatches(names, nodes(kinds))
    expect([...matches.yourMove]).toEqual(["conflict", "review", "approved"])
    expect([...matches.landing]).toEqual(["robot"])
    expect([...matches.waiting]).toEqual(["parent", "blocked"])
    expect([...matches.inMaster]).toEqual(["merged"])
    // a branch without a node yet is in All only
    expect(chipCount(matches, "all")).toBe(8)
    expect(chipCount(matches, "yourMove")).toBe(3)
  })

  it("orders the moves for F2 by urgency, a fix first, each kind in table order", () => {
    const kinds = nodes({ docs: "ready", auth: "yourStep", api: "needsFix", web: "waiting", cli: "needsFix", core: "ready" })
    expect(movesByUrgency(["docs", "auth", "api", "web", "cli", "core"], kinds)).toEqual(["api", "cli", "auth", "docs", "core"])
    expect(movesByUrgency(["web"], kinds)).toEqual([])
  })

  it("shows the branches of the chosen chip that match the text, in table order; an inactive filter shows all", () => {
    const branches = [branch("auth"), branch("api", { remoteStatus: testRemoteStatus() }), branch("docs", {}, "Document the API")]
    const matches = collectChipMatches(
      branches.map(it => it.name),
      nodes({ auth: "ready", api: "waiting", docs: "yourStep" }),
    )
    const byName = Object.fromEntries(branches.map(it => [it.name, it]))
    const filter = (patch: Partial<BranchFilter>) => filterBranches(matches, byName, { ...NO_FILTER, ...patch })
    expect(filter({})).toBeNull()
    expect(isFilterActive({ text: " ", chip: "all" })).toBe(false)
    expect(filter({ text: "api" })).toEqual(["api", "docs"])
    expect(filter({ chip: "yourMove" })).toEqual(["auth", "docs"])
    expect(filter({ text: "api", chip: "yourMove" })).toEqual(["docs"])
    expect(filter({ text: "gone" })).toEqual([])
  })

  it("keeps the branches shown since the filter changed, and adds the new matches", () => {
    const first = keepShown(null, ["auth", "api"])
    expect([...first]).toEqual(["auth", "api"])
    // auth's Safe Push started: it no longer matches Your Move, but stays
    expect(keepShown(first, ["api"])).toBe(first)
    expect([...keepShown(first, ["api", "docs"])]).toEqual(["auth", "api", "docs"])
  })

  it("says what found nothing", () => {
    expect(formatNoMatch({ text: " foo ", chip: "all" })).toBe("No branches match “foo”")
    expect(formatNoMatch({ text: "foo", chip: "yourMove" })).toBe("No branches of your move match “foo”")
    expect(formatNoMatch({ text: "", chip: "yourMove" })).toBe("Nothing is your move")
    expect(formatNoMatch({ text: "", chip: "landing" })).toBe("Nothing is landing")
    expect(formatNoMatch({ text: "", chip: "waiting" })).toBe("Nothing waits for others")
    expect(formatNoMatch({ text: "", chip: "inMaster" })).toBe("Nothing is in master")
  })
})
