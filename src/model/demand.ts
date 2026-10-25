/**
 * What this window shows and needs of the session of its repository (docs/design/rust-domain-authority.md, "Demand"):
 * the one thing the page tells Rust of itself (`set_demand`, by the window's subscription), which the session asks its
 * facts by. A surface holds a part while it shows it (`showBranch`, `showStack`, `needBranch`, `previewBranch`,
 * `showReviewWindow`, `showRobotDetails`): each answers its release and is counted, so two surfaces of one branch hold it
 * once; a watched robot of no branch is held for the life of the window (`watchRobotInDemand`). The focus of the window
 * and whether the machine is online come with them.
 *
 * A change goes out at most four times a second (`DEMAND_SEND_INTERVAL_MS`), without waiting for an answer, and only
 * when it differs from what was sent; a new subscription (another repository, a session opened again) gets it again.
 * The session asks the robots by it (the cadence of the poll by the focus, the watched robots polled with those of the
 * branches); the other stores still ask their facts themselves until each family moves into the session.
 */
import type { Demand } from "@/lib/bindings"
import { commands } from "@/lib/bindings"

/** The least time between two sends of the demand */
export const DEMAND_SEND_INTERVAL_MS = 250

type Part = "visibleBranches" | "visibleStacks" | "needed" | "previewed" | "reviewWindows" | "robotDetails" | "watchedRobots"

const PARTS: readonly Part[] = ["visibleBranches", "visibleStacks", "needed", "previewed", "reviewWindows", "robotDetails", "watchedRobots"]

/** By part, each name with the number of surfaces that hold it */
const held = new Map<Part, Map<string, number>>(PARTS.map(part => [part, new Map()]))
let focused = false
/** The subscription of the window to its session; null without one: nothing is sent */
let subscription: number | null = null
/** What was sent last, with its subscription; null: nothing yet to this subscription */
let lastSent: string | null = null
let lastSentAt = Number.NEGATIVE_INFINITY
let timer: ReturnType<typeof setTimeout> | null = null
let listensOnline = false

function hold(part: Part, name: string): () => void {
  const counts = held.get(part)!
  counts.set(name, (counts.get(name) ?? 0) + 1)
  schedule()
  let released = false
  return () => {
    if (released) {
      return
    }
    released = true
    const left = (counts.get(name) ?? 1) - 1
    if (left === 0) {
      counts.delete(name)
    } else {
      counts.set(name, left)
    }
    schedule()
  }
}

/** The row of the branch is on the screen until the returned function is called */
export function showBranch(branchName: string): () => void {
  return hold("visibleBranches", branchName)
}

/** The island of the whole stack of `root` is on the screen until the returned function is called */
export function showStack(root: string): () => void {
  return hold("visibleStacks", root)
}

/** A surface needs the facts of the branch now (the Safe Push popover, the "Create pull request" dialog) until the returned function is called */
export function needBranch(branchName: string): () => void {
  return hold("needed", branchName)
}

/** The preview of the branch is open until the returned function is called */
export function previewBranch(branchName: string): () => void {
  return hold("previewed", branchName)
}

/** This window is the review window of the branch until the returned function is called */
export function showReviewWindow(branchName: string): () => void {
  return hold("reviewWindows", branchName)
}

/** A surface shows the details of the robot until the returned function is called */
export function showRobotDetails(robotId: string): () => void {
  return hold("robotDetails", robotId)
}

/**
 * The window follows the robot of no branch it started (the Safe Push of a reviewed pull request) until it finishes: the
 * session polls it with the robots of the branches, for the life of the window
 */
export function watchRobotInDemand(robotId: string): void {
  if (held.get("watchedRobots")!.has(robotId)) {
    return
  }
  hold("watchedRobots", robotId)
}

/** The window gained (`true`) or lost the focus */
export function setDemandFocused(value: boolean): void {
  if (focused !== value) {
    focused = value
    schedule()
  }
}

/** The window's subscription to its session, which the demand goes to: a new one gets it again; null without one */
export function setDemandSubscription(id: number | null): void {
  if (subscription === id) {
    return
  }
  subscription = id
  lastSent = null
  listenOnline()
  schedule()
}

/** The demand of the window as it holds it now, each list sorted */
export function currentDemand(): Demand {
  const names = (part: Part) => [...held.get(part)!.keys()].sort()
  return {
    focused,
    online: typeof navigator === "undefined" || navigator.onLine,
    visibleBranches: names("visibleBranches"),
    visibleStacks: names("visibleStacks"),
    needed: names("needed"),
    previewed: names("previewed"),
    reviewWindows: names("reviewWindows"),
    robotDetails: names("robotDetails"),
    watchedRobots: names("watchedRobots"),
  }
}

function listenOnline(): void {
  if (listensOnline || typeof window === "undefined") {
    return
  }
  listensOnline = true
  window.addEventListener("online", schedule)
  window.addEventListener("offline", schedule)
}

/** Sends the demand as soon as the interval since the last send allows; a send already due takes this change too */
function schedule(): void {
  if (timer !== null || subscription === null) {
    return
  }
  const wait = Math.max(0, lastSentAt + DEMAND_SEND_INTERVAL_MS - Date.now())
  timer = setTimeout(send, wait)
}

function send(): void {
  timer = null
  const target = subscription
  if (target === null) {
    return
  }
  const demand = currentDemand()
  const key = JSON.stringify([target, demand])
  if (key === lastSent) {
    return
  }
  lastSent = key
  lastSentAt = Date.now()
  commands.setDemand({ subscription: target, demand }).catch((error: unknown) => console.error("Cannot tell the session what the window shows", error))
}

/** Forgets everything the window held (a test between its cases) */
export function resetDemand(): void {
  for (const counts of held.values()) {
    counts.clear()
  }
  focused = false
  subscription = null
  lastSent = null
  lastSentAt = Number.NEGATIVE_INFINITY
  if (timer !== null) {
    clearTimeout(timer)
    timer = null
  }
}
