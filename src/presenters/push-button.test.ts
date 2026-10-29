import { describe, expect, it } from "vite-plus/test"
import type { ActionBlocker, DroppedCommit, HoldKind, RunningOperation } from "@/lib/bindings"
import type { PushButtonContext } from "@/presenters/push-button"
import { getPushButtonState, isFirstPushInLanding } from "@/presenters/push-button"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { CREATE, forcePush, PUSH, testBranch, testRemoteStatus } from "@/stores/test-branch"

function commits(count: number): DroppedCommit[] {
  return Array.from({ length: count }, (_, index) => ({ id: `c${index}`, subject: `Fix ${index}`, author: "Ann", authorEmail: "ann@example.com", authorTime: 0 }))
}

/**
 * The push button of `login` with `remote`, as the view has it: Rust's blocker of the push (`up to date` for a remote
 * that has it all, as `domain_policy::actions` says it, unless `context` names another) and Push with Children enabled
 */
function state(remote: Partial<RemoteStatus>, context: PushButtonContext = {}) {
  const remoteStatus = testRemoteStatus(remote)
  const blocker: ActionBlocker | null = remoteStatus.action.kind === "upToDate" ? { code: "upToDate" } : null
  return getPushButtonState(testBranch("login", null, { stack: null, remoteStatus }), { blocker, childrenEnabled: true, ...context })
}

/** Rust's refusal of the push of `branch`: the Safe Push of `by` holds it (`kind`), or a merged one of it landed it */
function held(kind: HoldKind, branch: string, by: string): ActionBlocker {
  const hold = { kind, by: { branch: by, robotId: "r", wholeStack: false }, carried: null }
  return { code: "rule", reason: { code: "held", branch, hold, mode: "rebaseAutosquash", target: "master" } }
}

const oauth = testBranch("oauth", "login", { remoteStatus: testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1, action: CREATE }) })

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
    const push = state({
      unpushedCommits: ["a"],
      commitsAhead: 1,
      myCommitsAhead: 1,
      commitsBehind: 12,
      fastForward: false,
      droppedCommits: commits(10),
      action: forcePush({ drops: true }),
    })
    expect(push.text).toBe("Force Push Branch (1)")
    expect(push.tooltip).toBe("Force push replaces the remote branch.\n1 commit to push.\nThe remote has 12 commits your branch doesn't (by Ann).")
    expect(push.dropped).toEqual({ commits: commits(10), more: 2 })
    expect(state({ commitsBehind: 2, fastForward: false, droppedCommits: commits(2), action: forcePush({ drops: true }) }).dropped).toEqual({ commits: commits(2), more: 0 })
  })

  it("lists nothing for a push that drops no remote commit", () => {
    // a rebased branch: the push replaces the remote commits with their copies
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, fastForward: false, action: forcePush() }).dropped).toBeNull()
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, action: PUSH }).dropped).toBeNull()
    expect(state({ exists: false, commitsAhead: 1, action: CREATE }).dropped).toBeNull()
  })

  it("counts the commits the push sends in every label, the user's share in the description", () => {
    // the count of the compact button in the row
    expect(state({ commitsAhead: 3, myCommitsAhead: 2, action: PUSH })).toMatchObject({ text: "Push Branch (3)", count: 3, tooltip: "2 of 3 commits to push are yours." })
    expect(state({ exists: false, commitsAhead: 3, myCommitsAhead: 0, action: CREATE })).toMatchObject({
      text: "Push Branch (3)",
      tooltip: "3 commits to push, none of them yours.\nThe branch isn't on the remote yet: the push creates it.",
    })
    expect(state({ commitsAhead: 3, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, droppedCommits: commits(1), action: forcePush({ drops: true }) })).toMatchObject({
      text: "Force Push Branch (3)",
      tooltip: "Force push replaces the remote branch.\n1 of 3 commits to push is yours.\nThe remote has 1 commit your branch doesn't (by Ann).",
    })
    // a force push that sends nothing only drops the remote commits
    expect(state({ commitsBehind: 2, fastForward: false, droppedCommits: commits(2), action: forcePush({ drops: true }) })).toMatchObject({
      text: "Force Push Branch",
      count: 0,
      tooltip: "Force push replaces the remote branch.\nThe remote has 2 commits your branch doesn't (by Ann).",
    })
  })

  it("says what a force push of an amended or restacked branch replaces, and pushes it on the click in the push colour", () => {
    // when it was pushed stays last
    expect(state({ commitsRestacked: 1, fastForward: false, lastPushTime: 1704139200, action: forcePush({ replaces: true }) }).tooltip).toMatch(
      /^Force push replaces the older versions of the parent's commits\.\n\nPushed /,
    )
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, fastForward: false, action: forcePush({ replaces: true }) })).toMatchObject({
      text: "Force Push Branch (1)",
      color: "primary",
      tooltip: "Force push replaces 1 commit amended here.\n1 commit to push.",
      dropped: null,
      confirm: false,
    })
    // nothing of its own to push: the remote has its parent's commits as they were
    const restacked = testBranch("oauth", "login", { remoteStatus: testRemoteStatus({ commitsRestacked: 2, fastForward: false, action: forcePush({ replaces: true }) }) })
    expect(getPushButtonState(restacked)).toMatchObject({
      text: "Force Push Branch",
      color: "primary",
      tooltip: "Force push replaces the older versions of login's commits.",
      dropped: null,
      confirm: false,
      disabled: false,
    })
    // someone else's commit beside an amended one: the push drops it, and asks first
    expect(
      state({
        commitsAhead: 1,
        myCommitsAhead: 1,
        commitsAmended: 1,
        commitsBehind: 1,
        fastForward: false,
        droppedCommits: commits(1),
        action: forcePush({ drops: true, replaces: true }),
      }),
    ).toMatchObject({
      color: "warning",
      tooltip:
        "Force push replaces the remote branch.\n1 commit to push.\nThe remote has an older version of 1 commit amended here.\nThe remote has 1 commit your branch doesn't (by Ann).",
      dropped: { commits: commits(1), more: 0 },
      confirm: true,
    })
  })

  it("says that everything is pushed, and when, for an up to date branch", () => {
    expect(state({})).toMatchObject({ text: "Push Branch", count: 0, tooltip: "Everything is pushed.", disabled: true })
    expect(state({ lastPushTime: 1704139200 }).tooltip).toMatch(/^Everything is pushed\.\n\nPushed /)
  })

  it("asks first only for a push that drops remote commits", () => {
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 2, fastForward: false, droppedCommits: commits(2), action: forcePush({ drops: true }) }).confirm).toBe(true)
    expect(state({ commitsBehind: 1, fastForward: false, droppedCommits: commits(1), action: forcePush({ drops: true }) }).confirm).toBe(true)
    // a rebased branch only replaces the remote commits with their copies; a plain push and a first push drop nothing
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, fastForward: false, action: forcePush() }).confirm).toBe(false)
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, action: PUSH }).confirm).toBe(false)
    expect(state({ exists: false, commitsAhead: 1, action: CREATE }).confirm).toBe(false)
  })

  it("is quiet for a branch the baseline has all of: the update archives it", () => {
    expect(state({ exists: false, commitsAhead: 1, myCommitsAhead: 1, action: CREATE }, { landedIn: "origin/master" })).toMatchObject({
      text: "Push Branch (1)",
      color: "neutral",
      tooltip: "Already in origin/master. Update to archive the branch.",
      disabled: false,
      action: null,
    })
  })

  it("pushes the stack while an ancestor has work for the remote, and offers the push of the branch alone", () => {
    const parent = login({ exists: false, commitsAhead: 2, myCommitsAhead: 2, action: CREATE })
    const push = stacked({ unpushedParent: { name: "login", onRemote: false, stackSize: 2, buttonStack: [parent, oauth] } })
    expect(push).toMatchObject({
      text: "Push Stack (2 branches)",
      count: 3,
      color: "primary",
      disabled: false,
      confirm: false,
      pushesStack: true,
      tooltip: "Pushes 2 branches, parents first:\nlogin · 2 commits · creates the remote branch\noauth · 1 commit · creates the remote branch",
    })
    expect(push.action).toMatchObject({
      description: "login isn't on the remote yet. Pushing oauth alone works: its pull request includes login's commits either way.",
      label: "Push oauth Only",
    })
    const rebased = login({ commitsAhead: 1, myCommitsAhead: 1, fastForward: false, action: forcePush() })
    const onRemote = stacked({ unpushedParent: { name: "login", onRemote: true, stackSize: 2, buttonStack: [rebased, oauth] } })
    expect(onRemote.tooltip).toBe("Pushes 2 branches, parents first:\nlogin · 1 commit · force push\noauth · 1 commit · creates the remote branch")
    expect(onRemote.action?.description).toBe(
      "login has commits the remote lacks. Pushing oauth alone works: its pull request includes them either way, and the remote branch of login stays as it is.",
    )
    expect(stacked({}).action).toBeNull()
    expect(stacked({}).pushesStack).toBe(false)
  })

  it("says what each force push of a stack push replaces, as the push of the branch alone does", () => {
    const auth = testBranch("auth", null, {
      remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, fastForward: false, action: forcePush({ replaces: true }) }),
    })
    const restacked = login({ commitsRestacked: 1, fastForward: false, action: forcePush({ replaces: true }) })
    const push = stacked({ unpushedParent: { name: "login", onRemote: true, stackSize: 3, buttonStack: [auth, restacked, oauth] } })
    expect(push.tooltip).toBe(
      [
        "Pushes 3 branches, parents first:",
        "auth · 1 commit · force push, replaces 1 commit amended here",
        "login · force push, replaces the older versions of auth's commits",
        "oauth · 1 commit · creates the remote branch",
      ].join("\n"),
    )
  })

  it("takes the colour of an ancestor when none of the branch's commits are the user's", () => {
    const notMine = testBranch("oauth", "login", { remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 0, action: PUSH }) })
    const parent = login({ exists: false, commitsAhead: 1, myCommitsAhead: 1, action: CREATE })
    expect(stacked({ unpushedParent: { name: "login", onRemote: false, stackSize: 2, buttonStack: [parent, notMine] } }, notMine).color).toBe("primary")
    expect(getPushButtonState(notMine).color).toBe("neutral")
  })

  it("offers Push Stack while the button pushes the branch alone: its push drops remote commits, or an ancestor's does", () => {
    const dropping = testBranch("oauth", "login", {
      remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, droppedCommits: commits(1), action: forcePush({ drops: true }) }),
    })
    const push = stacked({ unpushedParent: { name: "login", onRemote: true, stackSize: 2, buttonStack: [] } }, dropping)
    expect(push).toMatchObject({ text: "Force Push Branch (1)", confirm: true, pushesStack: false })
    expect(push.action).toMatchObject({
      description: "login has commits the remote lacks. Pushing oauth alone works: its pull request includes them either way, and the remote branch of login stays as it is.",
      label: "Push Stack (2 branches)",
    })
    // login asks first in its own row: nothing above it to push with oauth
    expect(stacked({ unpushedParent: { name: "login", onRemote: true, stackSize: 2, buttonStack: [] } })).toMatchObject({ text: "Push Branch (1)", pushesStack: false })
  })

  it("says which pull requests into a virtual branch the push retargets first, and warns when GitHub may dismiss approvals", () => {
    const retarget = { branchName: "login", number: 8, base: "master", approved: true }
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, action: PUSH }, { retargets: [retarget] }).retarget).toEqual({
      text: "Retargets #8 to master: GitHub may dismiss its approvals.",
      warning: true,
    })
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, action: PUSH }, { retargets: [{ ...retarget, approved: false }] }).retarget).toEqual({
      text: "Retargets #8 to master.",
      warning: false,
    })
    // nothing to push: nothing is retargeted
    expect(state({}, { retargets: [retarget] }).retarget).toBeNull()

    // a stack push names the branch of each, and only of the branches it pushes
    const parent = login({ exists: false, commitsAhead: 2, myCommitsAhead: 2, action: CREATE })
    const retargets = [
      { branchName: "login", number: 801, base: "master", approved: false },
      { branchName: "oauth", number: 802, base: "master", approved: true },
    ]
    expect(stacked({ unpushedParent: { name: "login", onRemote: false, stackSize: 2, buttonStack: [parent, oauth] }, retargets }).retarget).toEqual({
      text: "Retargets #801 of login to master.\nRetargets #802 of oauth to master: GitHub may dismiss its approvals.",
      warning: true,
    })
    expect(stacked({ retargets }).retarget).toEqual({ text: "Retargets #802 to master: GitHub may dismiss its approvals.", warning: true })
  })

  it("offers Push with Children while branches stacked on it have work for the remote", () => {
    const push = state({ exists: false, commitsAhead: 1, myCommitsAhead: 1, action: CREATE }, { unpushedChildren: { names: ["oauth"], pushOrder: ["login", "oauth"] } })
    // the push alone stays as it is
    expect(push).toMatchObject({ text: "Push Branch (1)", disabled: false, action: null })
    expect(push.childrenAction).toMatchObject({ description: "oauth is stacked on login and has commits to push.", label: "Push with Children (2 branches)" })
    expect(
      state({ commitsAhead: 1, action: PUSH }, { unpushedChildren: { names: ["oauth", "session"], pushOrder: ["login", "oauth", "session"] } }).childrenAction?.description,
    ).toBe("oauth and session are stacked on login and have commits to push.")
    expect(
      state({ commitsAhead: 1, action: PUSH }, { unpushedChildren: { names: ["oauth", "session", "guide"], pushOrder: ["login", "oauth", "session", "guide"] } }).childrenAction
        ?.description,
    ).toBe("oauth and 2 more branches are stacked on login and have commits to push.")
    // one child: named, "(1 branch)" would say nothing
    expect(state({ commitsAhead: 1, action: PUSH }, { unpushedChildren: { names: ["oauth"], pushOrder: ["oauth"] } }).childrenAction?.label).toBe("Push with Children (oauth)")
    expect(state({ commitsAhead: 1, action: PUSH }).childrenAction).toBeNull()
  })

  it("offers Push with Children while the branch itself is on the remote: its own push is disabled, this one isn't", () => {
    const push = state({}, { unpushedChildren: { names: ["oauth"], pushOrder: ["oauth"] } })
    expect(push).toMatchObject({ disabled: true, count: 0, tooltip: "Everything is pushed." })
    expect(push.childrenAction).toMatchObject({ description: "oauth is stacked on login and has commits to push.", label: "Push with Children (oauth)" })
    // a branch the update archives: nothing of the user's left with it
    expect(state({}, { unpushedChildren: { names: ["oauth"], pushOrder: ["oauth"] }, landedIn: "origin/master" }).childrenAction).toBeNull()
  })

  it("names the stack branch of the whole stack that Push with Children of a root takes along", () => {
    const stackBranch = "user/stack/login"
    expect(state({}, { unpushedChildren: { names: ["oauth"], pushOrder: ["oauth"], stackBranch } }).childrenAction).toMatchObject({
      description: "oauth is stacked on login and has commits to push. The whole stack branch user/stack/login goes along.",
      label: "Push with Children (2 branches)",
    })
    // the branches on the remote, the stack branch behind them
    expect(state({}, { unpushedChildren: { names: [], pushOrder: [], stackBranch } }).childrenAction).toMatchObject({
      description: "The whole stack branch user/stack/login has commits to push.",
      label: "Push with Children (the whole stack branch)",
    })
  })

  it("offers no push while a Safe Push holds the branch, and says why; the children of a landed one push without it", () => {
    // Rust refuses Push with Children while a running Safe Push holds the branch
    const own = state(
      { commitsAhead: 2, action: PUSH },
      { blocker: held("own", "login", "login"), childrenEnabled: false, unpushedChildren: { names: ["oauth"], pushOrder: ["login", "oauth"] } },
    )
    expect(own).toMatchObject({
      disabled: true,
      count: 0,
      tooltip: "A Safe Push of login is running: wait until it merges, or restart it on the current branch",
      action: null,
      childrenAction: null,
    })
    const span = state({ commitsAhead: 2, action: PUSH }, { blocker: held("span", "login", "oauth") })
    expect(span).toMatchObject({ disabled: true, tooltip: "A Safe Push of oauth is running: it lands login with it" })
    // a merged one landed it: its commits are in the target
    const landed = state({ commitsAhead: 2, action: PUSH }, { blocker: held("landed", "login", "oauth") })
    expect(landed).toMatchObject({ disabled: true, count: 0, tooltip: "Landed in master with oauth: nothing of it is left to push", childrenAction: null })
    // the branches built on a landed one push without it: its push offers theirs, as a running Safe Push's never does
    const children = { unpushedChildren: { names: ["session"], pushOrder: ["session"] } }
    expect(state({ commitsAhead: 2, action: PUSH }, { blocker: held("landed", "login", "oauth"), ...children }).childrenAction).toMatchObject({
      description: "session is stacked on login and has commits to push.",
      label: "Push with Children (session)",
    })
    // a push that runs keeps its progress
    expect(state({ commitsAhead: 2, action: PUSH }, { blocker: held("own", "login", "login"), isPushing: true }).tooltip).not.toContain("Safe Push")
  })

  it("says why Rust refuses the push of a branch that stays local or failed to sync, in the words of the action", () => {
    const parked: ActionBlocker = { code: "rule", reason: { code: "parked", branch: "login", with: null, note: "" } }
    expect(state({ commitsAhead: 2, action: PUSH }, { blocker: parked })).toMatchObject({ disabled: true, count: 0, tooltip: "Parked: Unpark it to push" })
    expect(state({ commitsAhead: 2, action: PUSH }, { blocker: { code: "syncFailed", problem: null } })).toMatchObject({ disabled: true, tooltip: "The branch failed to sync" })
    // a sync that rebuilds the branch: the description stays what the remote has
    expect(state({ commitsAhead: 2, action: PUSH }, { blocker: { code: "syncing" } })).toMatchObject({
      disabled: true,
      text: "Push Branch (2)",
      tooltip: "2 commits to push, none of them yours.",
    })
    // an operation of another process holds what the push needs: its words, before Rust is asked
    const operations = {
      bd: {
        id: "bd",
        intent: { kind: "land", subject: { kind: "branch", name: "login" }, mode: "rebaseAutosquash", profile: { kind: "default" } },
        resources: [{ kind: "ref", name: "user/virtual/login" }],
        step: { kind: "starting" },
        origin: { kind: "otherProcess", tool: "bd", pid: 4242 },
        moves: [],
      },
    } as const satisfies Record<string, RunningOperation>
    expect(state({ commitsAhead: 2, action: PUSH }, { blocker: { code: "busy", op: "bd" }, operations })).toMatchObject({ disabled: true, tooltip: "bd: Safe Push of login…" })
  })

  it("offers the push of the branch alone before Push with Children for a branch in the middle of a stack", () => {
    const parent = login({ commitsAhead: 1, myCommitsAhead: 1, action: PUSH })
    const push = stacked({
      unpushedParent: { name: "login", onRemote: true, stackSize: 2, buttonStack: [parent, oauth] },
      unpushedChildren: { names: ["device"], pushOrder: ["oauth", "device"] },
      childrenEnabled: true,
    })
    expect(push.action?.label).toBe("Push oauth Only")
    expect(push.childrenAction?.label).toBe("Push with Children (2 branches)")
  })
})

describe("isFirstPushInLanding", () => {
  const remote = (status: Partial<RemoteStatus>): Branch => testBranch("login", null, { stack: null, remoteStatus: testRemoteStatus(status) })

  it("leaves the first push of a branch to its landing where Safe Push or Create Pull Request pushes it", () => {
    expect(isFirstPushInLanding(remote({ exists: false, commitsAhead: 2, action: CREATE }), { landingPushes: true })).toBe(true)
  })

  it("keeps a push that updates the remote branch, and the first push where nothing else pushes", () => {
    // the remote branch is what the pull request and the other people see
    expect(isFirstPushInLanding(remote({ commitsAhead: 2, action: PUSH }), { landingPushes: true })).toBe(false)
    expect(isFirstPushInLanding(remote({ commitsAhead: 1, commitsBehind: 1, fastForward: false, action: forcePush({ drops: true }) }), { landingPushes: true })).toBe(false)
    // neither Patronus nor GitHub: the push is the one way to share the branch
    expect(isFirstPushInLanding(remote({ exists: false, commitsAhead: 2, action: CREATE }), { landingPushes: false })).toBe(false)
  })

  it("keeps the push of a branch a Safe Push holds: its description says why it won't land and offers the robot", () => {
    expect(isFirstPushInLanding(remote({ exists: false, commitsAhead: 2, action: CREATE }), { landingPushes: true, held: true })).toBe(false)
  })
})
