import { describe, expect, it } from "vite-plus/test"
import type { RewriteIntent, TableWitness } from "@/lib/bindings"
import { presentOtherProcess } from "@/presenters/operations"
import { presentRewrite } from "@/presenters/rewrite"

const WITNESS: TableWitness = { head: "h1", storeTips: { issueGroups: null, stackParents: null, parked: null } }

describe("presentRewrite", () => {
  it("words a rewrite by what it does to what it names", () => {
    const moves: RewriteIntent = { rewrite: "move", commitIds: ["a", "b"], targetBranch: "auth", witness: WITNESS }
    expect(presentRewrite(moves)).toEqual({
      label: "Moving 2 commits to auth",
      phrase: "a move of 2 commits to auth",
      failure: "Cannot move the commits to auth",
      done: "Moved 2 commits to auth",
    })
    expect(presentRewrite({ ...moves, commitIds: ["a"], targetBranch: null }).label).toBe("Moving 1 commit back")
    expect(presentRewrite({ rewrite: "unstack", branchName: "beta", witness: WITNESS }).failure).toBe("Cannot unstack beta")
    expect(
      presentRewrite({
        rewrite: "park",
        branches: [
          { branchName: "auth", note: "" },
          { branchName: "ui", note: "later" },
        ],
        witness: WITNESS,
      }).label,
    ).toBe("Parking auth, ui")
    expect(presentRewrite({ rewrite: "undo", token: { kind: "regroup", token: "h0..h1" } }).failure).toBe("Cannot undo")
    expect(presentRewrite({ rewrite: "moveHunk", from: "a1", to: "b2", path: "src/app.ts", witness: WITNESS }).label).toBe("Moving a hunk of src/app.ts")
    expect(presentRewrite({ rewrite: "group", plan: { branches: { auth: ["a1"] } }, witness: WITNESS }).done).toBe("Regrouped the commits")
  })

  it("names the rewrite of another process on its row", () => {
    const rename: RewriteIntent = { rewrite: "rename", branchName: "auth", newName: "login", witness: WITNESS }
    const run = presentOtherProcess({
      id: "op",
      intent: { kind: "rewrite", ...rename },
      resources: [{ kind: "head" }],
      step: { kind: "rewriting" },
      origin: { kind: "otherProcess", tool: "bd", pid: 4242 },
      moves: [],
    })
    expect(run?.label).toBe("bd: Renaming auth to login")
    expect(run?.description).toBe("`bd` (pid 4242) runs the rename of auth, which holds what it needs until it ends")
    const pick = presentOtherProcess({
      id: "op",
      intent: { kind: "pick", source: "../lease" },
      resources: [{ kind: "head" }],
      step: { kind: "rewriting" },
      origin: { kind: "otherProcess", tool: "bd", pid: 4242 },
      moves: [],
    })
    expect(pick?.label).toBe("bd: Picking commits")
  })
})
