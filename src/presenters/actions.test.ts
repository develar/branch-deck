// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ActionBlocker, ActionId, Reason, RunningOperation } from "@/lib/bindings"
import { ACTION_LOOKS, formatActionBlocker, getStartActionItems, presentAction, whileSyncing } from "@/presenters/actions"
import { formatStackHold } from "@/presenters/reasons"
import { disabled, enabled } from "@/test-support/actions"

/** Every action Rust offers and an example of every blocker it gives (`action_catalog_test.rs` writes it) */
const CATALOG = Object.values(
  import.meta.glob<{ ids: ActionId[]; blockers: [ActionId, ActionBlocker][] }>("/crates/domain-policy/actions.json", { eager: true, import: "default" }),
)[0]!

describe("the actions Rust offers", () => {
  it("presents every action of the catalog Rust writes by its id, with its look", () => {
    expect(CATALOG.ids.length).toBeGreaterThan(0)
    for (const id of CATALOG.ids) {
      const item = presentAction(enabled(id), id, { onSelect: () => {} }, { subject: "auth" })
      expect(item).toMatchObject({ id, label: ACTION_LOOKS[id].label, icon: ACTION_LOOKS[id].icon })
      expect(item?.disabled).toBeUndefined()
    }
    // the look covers no id Rust doesn't offer
    expect(Object.keys(ACTION_LOOKS).toSorted()).toEqual(CATALOG.ids.toSorted())
  })

  it("words every blocker of the catalog as the page says it", () => {
    const words = { subject: "auth", statusText: "merge conflict" }
    expect(CATALOG.blockers.map(([id, blocker]) => formatActionBlocker(id, blocker, words))).toEqual([
      "Parked: Unpark it to push",
      "Wait for the sync to finish",
      "Waits for GitHub to list the pull requests",
      "Patronus didn't tell why the Safe Push failed (502 Bad Gateway): Check Approvals Again asks again",
      "Wait for the sync to finish",
      "The branch failed to sync: merge conflict",
      "auth is in a stack cycle",
      "No commits to push",
      "No commits to review",
      "The branch has no commits",
      "The remote branch is up to date",
      "#7 is a draft: mark it ready for review on GitHub first",
      "A comment on #7 runs the default checks: it can't choose a profile",
      "Every commit of the branch has an issue reference",
      "A Safe Push of auth is running: wait until it merges, or restart it on the current branch: it lands whatever a park says",
      "login asks first in its row: the push drops remote commits",
      "Only auth has commits to push: its own push sends them",
      "Landed in master with deploy and side-test: nothing of auth and its children is left to push",
      "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack",
      "The branches of the stack are on the remote. The whole stack branch stays: its push would drop remote commits it lacks",
      "Nothing to push: the stack is on the remote",
      "Landed in origin/master",
      "Your own pull request: GitHub takes no review of its author",
      "You approved 01234567",
      "Reading the branch Patronus pushes into…",
      "Patronus pushes into a branch of origin: the baseline is none",
      "#101 merges into release/253: Patronus would merge it there, not into master",
    ])
  })

  it("says why Push with Children leaves the stack branch of the whole stack as it is, and names the children that ask first", () => {
    const words = { subject: "auth" }
    const nothing = (why: Extract<ActionBlocker, { code: "nothingBeyond" }>["why"]) => formatActionBlocker("push-with-children", { code: "nothingBeyond", why }, words)
    const conflict = { commit: "0123456789abcdef", files: ["a.kt"] }
    const notBuilt: Reason = { code: "stackNotBuilt", root: "auth", state: "Conflict", blockedBy: null, conflict, error: null }
    expect(nothing({ kind: "stackBranch", along: { kind: "notBuilt", reason: notBuilt } })).toBe(
      "The branches of the stack are on the remote. The stack branch conflicts at 01234567 in a.kt: the stack lands in pieces",
    )
    const held: Reason = { code: "stackHeld", root: "auth", robotId: "robot-1", mode: "rebaseAutosquash" }
    expect(nothing({ kind: "stackBranch", along: { kind: "held", reason: held } })).toBe(`The branches of the stack are on the remote. ${formatStackHold(held)}`)
    expect(nothing({ kind: "askFirst", names: ["login", "oauth"] })).toBe("login and oauth ask first in their rows: the push drops remote commits")
    expect(nothing({ kind: "landed", by: [], own: true, target: "master" })).toBe("Merged by Patronus: nothing of auth and its children is left to push")
  })

  it("disables an item with the words of its blocker, the step's for a reason of the next step, and offers none Rust doesn't", () => {
    const step: ActionBlocker = { code: "step", blocker: { code: "step" } }
    expect(
      presentAction([disabled("safe-push", step)], "safe-push", { label: "Safe Push · 3", onSelect: () => {} }, { subject: "auth", step: "Waits for the approval of Ann" }),
    ).toMatchObject({
      id: "safe-push",
      label: "Safe Push · 3",
      kbds: ["s"],
      disabled: true,
      description: "Waits for the approval of Ann",
    })
    expect(presentAction(enabled("push"), "dry-run", { onSelect: () => {} }, { subject: "auth" })).toBeNull()
    // a surface without the key leaves it out
    expect(presentAction(enabled("push"), "push", { onSelect: () => {}, kbds: null }, { subject: "auth" })?.kbds).toBeUndefined()
  })

  it("lays a sync of the table over a rewrite, keeping the reason Rust gave", () => {
    const rename = presentAction(enabled("rename"), "rename", { onSelect: () => {} }, { subject: "auth" })!
    expect(whileSyncing(rename, true)).toMatchObject({ disabled: true, description: "Wait for the sync to finish" })
    expect(whileSyncing(rename, false)).toBe(rename)
    const failed = presentAction([disabled("rename", { code: "noCommits" })], "rename", { onSelect: () => {} }, { subject: "auth" })!
    expect(whileSyncing(failed, true)).toMatchObject({ disabled: true, description: "The branch has no commits" })
  })
})

describe("the actions of a start whose answer was lost", () => {
  /** A Safe Push of auth whose answer was lost, sent through the API, or as a comment on #7 */
  function lost(pullRequest: number | null): RunningOperation {
    return {
      id: "01LOST",
      intent: { kind: "land", subject: { kind: "branch", name: "auth" }, mode: "rebaseAutosquash", profile: { kind: "default" } },
      resources: [],
      step: { kind: "startUnknown", sentAt: 1_800_000_000, url: "https://patronus.example", pullRequest },
      origin: { kind: "otherProcess", tool: "bd", pid: 4242 },
      moves: [],
    }
  }

  it("offers Open in Patronus where its robot would be and Forget This Start, which says Patronus may still run it", () => {
    const offered = [...enabled("open-unknown-start"), ...enabled("forget-start")]
    expect(getStartActionItems(lost(null), offered).map(item => [item.id, item.label, item.description ?? null])).toEqual([
      ["open-unknown-start", "Open in Patronus", null],
      ["forget-start", "Forget This Start", "Lets auth start again. Patronus may still run the start that was sent: its robot shows once Patronus lists it"],
    ])
    expect(getStartActionItems(lost(7), offered)[0]?.label).toBe("Open Pull Request #7")
    expect(getStartActionItems(lost(null), undefined)).toEqual([])
  })
})
