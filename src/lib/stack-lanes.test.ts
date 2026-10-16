import { describe, expect, it } from "vite-plus/test"
import type { StackLanes } from "@/lib/stack-lanes"
import { ancestorSegments, landingSegments, laneCountOf, lanesBelow, layoutStack, segmentLanesBelow } from "@/lib/stack-lanes"

/**
 * `name:lane`, then `<n` for the lane a side child forks from, `^` for the line up into the node, `v` for the line down
 * from it and `|0,1` for the lanes passing the row
 */
function format(name: string, { lane, forkFrom, up, down, through }: StackLanes): string {
  return `${name}:${lane}${forkFrom === null ? "" : `<${forkFrom}`}${up ? "^" : ""}${down ? "v" : ""}${through.length > 0 ? `|${through.join(",")}` : ""}`
}

/** The stack of `root` over a map of children in branch order (newest first) */
function graph(root: string, children: Record<string, string[]>): string[] {
  return layoutStack(root, name => children[name] ?? []).map(row => format(row.name, row.lanes))
}

describe("stack lanes", () => {
  it("keeps a straight chain on lane 0", () => {
    expect(graph("a", { a: ["b"], b: ["c"] })).toEqual(["a:0v", "b:0^v", "c:0^"])
  })

  it("continues the longest chain on the parent's lane and forks the others off it, shortest first", () => {
    const children = { root: ["main", "leaf", "side"], side: ["side2"], main: ["main2"], main2: ["main3"] }
    expect(graph("root", children)).toEqual([
      "root:0v",
      // the leaf sibling, then the side chain, each on lane 1 beside the line of root down to its main child
      "leaf:1<0|0",
      "side:1<0v|0",
      "side2:1^|0",
      "main:0^v",
      "main2:0^v",
      "main3:0^",
    ])
  })

  it("opens a lane per fork inside a side subtree, and frees it after the subtree", () => {
    const children = { root: ["side", "main"], side: ["inner", "leaf"], inner: ["inner2"], main: ["after"], after: ["late", "end"], end: ["end2"] }
    expect(graph("root", children)).toEqual([
      "root:0v",
      "side:1<0v|0",
      "leaf:2<1|0,1",
      "inner:1^v|0",
      "inner2:1^|0",
      "main:0^v",
      "after:0^v",
      // lanes 1 and 2 are free again
      "late:1<0|0",
      "end:0^v",
      "end2:0^",
    ])
  })

  it("keeps the given order of children with chains of the same length: the oldest is the main line", () => {
    expect(graph("root", { root: ["newer", "older"] })).toEqual(["root:0v", "newer:1<0|0", "older:0^"])
  })

  it("counts the lanes of the rows", () => {
    const children: Record<string, string[]> = { root: ["side", "main"], side: ["leaf", "inner"], inner: ["x"], main: ["m2"], m2: ["m3"], m3: ["m4"] }
    const rows = layoutStack("root", name => children[name] ?? [])
    expect(laneCountOf(rows.map(row => row.lanes))).toBe(3)
    expect(laneCountOf([null])).toBe(0)
  })

  it("runs the lanes passing a row on below it, and its own while it has children or shows its commits", () => {
    const side: StackLanes = { lane: 1, forkFrom: 0, up: false, down: false, through: [0] }
    expect(lanesBelow(side, false)).toEqual([0])
    expect(lanesBelow(side, true)).toEqual([0, 1])
    expect(lanesBelow({ ...side, down: true }, false)).toEqual([0, 1])
  })
})

/** auth with session (a side branch) and login, oauth on login: the table rows of the stack graph */
function stackRows(children: Record<string, string[]>, root = "auth") {
  return layoutStack(root, name => children[name] ?? []).map(({ name, lanes }) => ({ name, lanes }))
}

const STACK = { auth: ["session", "login"], login: ["oauth"] }

describe("landingSegments", () => {
  it("draws the span from the root down to the branch whose robot lands it, past the side branches between", () => {
    const rows = stackRows(STACK)
    expect(rows.map(it => it.name)).toEqual(["auth", "session", "login", "oauth"])
    const lanes = landingSegments(rows, new Map([["auth", "oauth"], ["login", "oauth"]]))
    expect(Object.fromEntries(lanes)).toEqual({
      oauth: { up: true, fork: false, down: false, through: [] },
      login: { up: true, fork: false, down: true, through: [] },
      // the lane of auth passes the side branch on its way to login
      session: { up: false, fork: false, down: false, through: [0] },
      auth: { up: false, fork: false, down: true, through: [] },
    })
    // the same segments are the same object, so the rows keep their props
    expect(landingSegments(rows, new Map([["auth", "oauth"], ["login", "oauth"]])).get("auth")).toBe(lanes.get("auth"))
  })

  it("curves into a side branch whose robot lands its parent, and leaves the main line alone", () => {
    const rows = stackRows(STACK)
    expect(Object.fromEntries(landingSegments(rows, new Map([["auth", "session"]])))).toEqual({
      session: { up: false, fork: true, down: false, through: [] },
      auth: { up: false, fork: false, down: true, through: [] },
    })
  })

  it("stops where the graph doesn't connect the branches of a span", () => {
    // login is a root of its own here (its parent has no row): nothing connects it to auth
    const rows = [...stackRows({}, "auth"), ...stackRows({ login: ["oauth"] }, "login")]
    expect(Object.fromEntries(landingSegments(rows, new Map([["auth", "oauth"], ["login", "oauth"]])))).toEqual({
      oauth: { up: true, fork: false, down: false, through: [] },
      login: { up: false, fork: false, down: true, through: [] },
    })
    expect(landingSegments(rows, new Map()).size).toBe(0)
  })

  it("runs the highlighted lanes on below a row, its own while its line goes down", () => {
    const [auth, session] = stackRows(STACK)
    const lanes = landingSegments(stackRows(STACK), new Map([["auth", "oauth"], ["login", "oauth"]]))
    expect(segmentLanesBelow(auth!.lanes, lanes.get("auth")!)).toEqual([0])
    expect(segmentLanesBelow(session!.lanes, lanes.get("session")!)).toEqual([0])
    expect(segmentLanesBelow(session!.lanes, null)).toEqual([])
  })
})

describe("ancestorSegments", () => {
  it("draws the path from the root down to a branch, past the side branches between", () => {
    const rows = stackRows(STACK)
    // the span a Safe Push of oauth lands
    expect(ancestorSegments(rows, "oauth")).toEqual(landingSegments(rows, new Map([["auth", "oauth"], ["login", "oauth"]])))
    expect(ancestorSegments(rows, "oauth").get("session")).toEqual({ up: false, fork: false, down: false, through: [0] })
  })

  it("curves into a side branch and leaves the main line alone", () => {
    const rows = stackRows(STACK)
    expect(ancestorSegments(rows, "session")).toEqual(landingSegments(rows, new Map([["auth", "session"]])))
    expect([...ancestorSegments(rows, "session").keys()].sort()).toEqual(["auth", "session"])
  })

  it("has no path for a root or a branch without a row", () => {
    const rows = stackRows(STACK)
    expect(ancestorSegments(rows, "auth").size).toBe(0)
    expect(ancestorSegments(rows, "gone").size).toBe(0)
    expect(ancestorSegments(rows, null).size).toBe(0)
  })
})
