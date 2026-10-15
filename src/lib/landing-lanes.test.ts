import { describe, expect, it } from "vitest"
import { highlightedLanesBelow, landingLanes } from "@/lib/landing-lanes"
import { layoutStack } from "@/lib/stack-lanes"

/** auth with session (a side branch) and login, oauth on login: the table rows of the stack graph */
function stackRows(children: Record<string, string[]>, root = "auth") {
  return layoutStack(root, name => children[name] ?? []).map(({ name, lanes }) => ({ name, lanes }))
}

const STACK = { auth: ["session", "login"], login: ["oauth"] }

describe("landingLanes", () => {
  it("draws the span from the root down to the branch whose robot lands it, past the side branches between", () => {
    const rows = stackRows(STACK)
    expect(rows.map(it => it.name)).toEqual(["auth", "session", "login", "oauth"])
    const lanes = landingLanes(rows, new Map([["auth", "oauth"], ["login", "oauth"]]))
    expect(Object.fromEntries(lanes)).toEqual({
      oauth: { up: true, fork: false, down: false, through: [] },
      login: { up: true, fork: false, down: true, through: [] },
      // the lane of auth passes the side branch on its way to login
      session: { up: false, fork: false, down: false, through: [0] },
      auth: { up: false, fork: false, down: true, through: [] },
    })
    // the same segments are the same object, so the rows keep their props
    expect(landingLanes(rows, new Map([["auth", "oauth"], ["login", "oauth"]])).get("auth")).toBe(lanes.get("auth"))
  })

  it("curves into a side branch whose robot lands its parent, and leaves the main line alone", () => {
    const rows = stackRows(STACK)
    expect(Object.fromEntries(landingLanes(rows, new Map([["auth", "session"]])))).toEqual({
      session: { up: false, fork: true, down: false, through: [] },
      auth: { up: false, fork: false, down: true, through: [] },
    })
  })

  it("stops where the graph doesn't connect the branches of a span", () => {
    // login is a root of its own here (its parent has no row): nothing connects it to auth
    const rows = [...stackRows({}, "auth"), ...stackRows({ login: ["oauth"] }, "login")]
    expect(Object.fromEntries(landingLanes(rows, new Map([["auth", "oauth"], ["login", "oauth"]])))).toEqual({
      oauth: { up: true, fork: false, down: false, through: [] },
      login: { up: false, fork: false, down: true, through: [] },
    })
    expect(landingLanes(rows, new Map()).size).toBe(0)
  })

  it("runs the highlighted lanes on below a row, its own while its line goes down", () => {
    const [auth, session] = stackRows(STACK)
    const lanes = landingLanes(stackRows(STACK), new Map([["auth", "oauth"], ["login", "oauth"]]))
    expect(highlightedLanesBelow(auth!.lanes, lanes.get("auth")!)).toEqual([0])
    expect(highlightedLanesBelow(session!.lanes, lanes.get("session")!)).toEqual([0])
    expect(highlightedLanesBelow(session!.lanes, null)).toEqual([])
  })
})
