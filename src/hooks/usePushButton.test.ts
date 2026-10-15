import { describe, expect, it } from "vitest"
import type { PushButtonContext } from "@/hooks/usePushButton"
import { getPushButtonState } from "@/hooks/usePushButton"
import type { DroppedCommit } from "@/lib/bindings"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

function commits(count: number): DroppedCommit[] {
  return Array.from({ length: count }, (_, index) => ({ id: `c${index}`, subject: `Fix ${index}`, author: "Ann", authorTime: 0 }))
}

function state(remote: Partial<RemoteStatus>, context: PushButtonContext = {}) {
  return getPushButtonState(testBranch("login", null, { stack: null, remoteStatus: testRemoteStatus(remote) }), context)
}

const oauth = testBranch("oauth", "login", { remoteStatus: testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1 }) })

/** oauth on login, not on the remote yet, with one commit of its own */
function stacked(context: PushButtonContext, branch: Branch = oauth) {
  return getPushButtonState(branch, context)
}

/** login with `remote`, the parent of oauth */
function login(remote: Partial<RemoteStatus>): Branch {
  return testBranch("login", "auth", { remoteStatus: testRemoteStatus(remote) })
}

describe("getPushButtonState", () => {
  it("lists the remote commits a force push drops under its description, with how many more there are", () => {
    const push = state({ unpushedCommits: ["a"], commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 12, fastForward: false, droppedCommits: commits(10) })
    expect(push.text).toBe("Force Push (1)")
    expect(push.tooltip).toBe("Force push replaces the remote branch.\n1 commit to push.\nThe remote has 12 commits your branch doesn't (by Ann).")
    expect(push.dropped).toEqual({ commits: commits(10), more: 2 })
    expect(state({ commitsBehind: 2, fastForward: false, droppedCommits: commits(2) }).dropped).toEqual({ commits: commits(2), more: 0 })
  })

  it("lists nothing for a push that drops no remote commit", () => {
    // a rebased branch: the push replaces the remote commits with their copies
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, fastForward: false }).dropped).toBeNull()
    expect(state({ commitsAhead: 1, myCommitsAhead: 1 }).dropped).toBeNull()
    expect(state({ exists: false, commitsAhead: 1 }).dropped).toBeNull()
  })

  it("counts the commits the push sends in every label, the user's share in the description", () => {
    expect(state({ commitsAhead: 3, myCommitsAhead: 2 })).toMatchObject({ text: "Push (3)", tooltip: "2 of 3 commits to push are yours." })
    expect(state({ exists: false, commitsAhead: 3, myCommitsAhead: 0 })).toMatchObject({
      text: "Create Remote (3)",
      tooltip: "3 commits to push, none of them yours.\nThe branch isn't on the remote yet: the push creates it.",
    })
    expect(state({ commitsAhead: 3, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, droppedCommits: commits(1) })).toMatchObject({
      text: "Force Push (3)",
      tooltip: "Force push replaces the remote branch.\n1 of 3 commits to push is yours.\nThe remote has 1 commit your branch doesn't (by Ann).",
    })
    // a force push that sends nothing only drops the remote commits
    expect(state({ commitsBehind: 2, fastForward: false, droppedCommits: commits(2) })).toMatchObject({
      text: "Force Push",
      tooltip: "Force push replaces the remote branch.\nThe remote has 2 commits your branch doesn't (by Ann).",
    })
  })

  it("says that everything is pushed, and when, for an up to date branch", () => {
    expect(state({})).toMatchObject({ text: "Push", tooltip: "Everything is pushed.", disabled: true })
    expect(state({ lastPushTime: 1704139200 }).tooltip).toMatch(/^Everything is pushed\.\n\nPushed /)
  })

  it("asks first only for a push that drops remote commits", () => {
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 2, fastForward: false, droppedCommits: commits(2) }).confirm).toBe(true)
    expect(state({ commitsBehind: 1, fastForward: false, droppedCommits: commits(1) }).confirm).toBe(true)
    // a rebased branch only replaces the remote commits with their copies; a plain push and a first push drop nothing
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, fastForward: false }).confirm).toBe(false)
    expect(state({ commitsAhead: 1, myCommitsAhead: 1 }).confirm).toBe(false)
    expect(state({ exists: false, commitsAhead: 1 }).confirm).toBe(false)
  })

  it("is quiet for a branch the baseline has all of: the update archives it", () => {
    expect(state({ exists: false, commitsAhead: 1, myCommitsAhead: 1 }, { landedIn: "origin/master" })).toMatchObject({
      text: "Create Remote (1)",
      color: "neutral",
      tooltip: "Already in origin/master. Update to archive the branch.",
      disabled: false,
      action: null,
    })
  })

  it("pushes the stack while an ancestor has work for the remote, and offers the push of the branch alone", () => {
    const parent = login({ exists: false, commitsAhead: 2, myCommitsAhead: 2 })
    const push = stacked({ unpushedParent: { name: "login", onRemote: false, stackSize: 2, buttonStack: [parent, oauth] } })
    expect(push).toMatchObject({
      text: "Push Stack (3)",
      color: "primary",
      disabled: false,
      confirm: false,
      pushesStack: true,
      tooltip: "Pushes 2 branches, parents first:\nlogin · 2 commits · creates the remote branch\noauth · 1 commit · creates the remote branch",
    })
    expect(push.action).toMatchObject({
      description: "login isn't on the remote yet. Pushing oauth alone works; a pull request includes login's commits until login is pushed.",
      label: "Push oauth Only",
    })
    const rebased = login({ commitsAhead: 1, myCommitsAhead: 1, fastForward: false })
    const onRemote = stacked({ unpushedParent: { name: "login", onRemote: true, stackSize: 2, buttonStack: [rebased, oauth] } })
    expect(onRemote.tooltip).toBe("Pushes 2 branches, parents first:\nlogin · 1 commit · force push\noauth · 1 commit · creates the remote branch")
    expect(onRemote.action?.description).toBe("login has commits the remote lacks. Pushing oauth alone works; a pull request includes them until login is pushed.")
    expect(stacked({}).action).toBeNull()
    expect(stacked({}).pushesStack).toBe(false)
  })

  it("takes the colour of an ancestor when none of the branch's commits are the user's", () => {
    const notMine = testBranch("oauth", "login", { remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 0 }) })
    const parent = login({ exists: false, commitsAhead: 1, myCommitsAhead: 1 })
    expect(stacked({ unpushedParent: { name: "login", onRemote: false, stackSize: 2, buttonStack: [parent, notMine] } }, notMine).color).toBe("primary")
    expect(getPushButtonState(notMine).color).toBe("neutral")
  })

  it("offers Push Stack while the button pushes the branch alone: its push drops remote commits, or an ancestor's does", () => {
    const dropping = testBranch("oauth", "login", { remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, droppedCommits: commits(1) }) })
    const push = stacked({ unpushedParent: { name: "login", onRemote: true, stackSize: 2, buttonStack: [] } }, dropping)
    expect(push).toMatchObject({ text: "Force Push (1)", confirm: true, pushesStack: false })
    expect(push.action).toMatchObject({
      description: "login has commits the remote lacks. Pushing oauth alone works; a pull request includes them until login is pushed.",
      label: "Push Stack (2 branches)",
    })
    // login asks first in its own row: nothing above it to push with oauth
    expect(stacked({ unpushedParent: { name: "login", onRemote: true, stackSize: 2, buttonStack: [] } })).toMatchObject({ text: "Create Remote (1)", pushesStack: false })
  })

  it("offers Push with Children while branches stacked on it have work for the remote", () => {
    const push = state({ exists: false, commitsAhead: 1, myCommitsAhead: 1 }, { unpushedChildren: { names: ["oauth"], pushCount: 2 } })
    // the push alone stays as it is
    expect(push).toMatchObject({ text: "Create Remote (1)", disabled: false, action: null })
    expect(push.childrenAction).toMatchObject({ description: "oauth is stacked on login and has commits to push.", label: "Push with Children (2 branches)" })
    expect(state({ commitsAhead: 1 }, { unpushedChildren: { names: ["oauth", "session"], pushCount: 3 } }).childrenAction?.description)
      .toBe("oauth and session are stacked on login and have commits to push.")
    expect(state({ commitsAhead: 1 }, { unpushedChildren: { names: ["oauth", "session", "guide"], pushCount: 4 } }).childrenAction?.description)
      .toBe("oauth and 2 more branches are stacked on login and have commits to push.")
    // an up to date branch offers it only in its menu: its button is disabled
    expect(state({}, { unpushedChildren: { names: ["oauth"], pushCount: 1 } }).childrenAction).toBeNull()
    expect(state({ commitsAhead: 1 }).childrenAction).toBeNull()
  })

  it("offers the push of the branch alone before Push with Children for a branch in the middle of a stack", () => {
    const parent = login({ commitsAhead: 1, myCommitsAhead: 1 })
    const push = stacked({ unpushedParent: { name: "login", onRemote: true, stackSize: 2, buttonStack: [parent, oauth] }, unpushedChildren: { names: ["device"], pushCount: 2 } })
    expect(push.action?.label).toBe("Push oauth Only")
    expect(push.childrenAction?.label).toBe("Push with Children (2 branches)")
  })
})
