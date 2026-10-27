/**
 * What this window shows and needs of the session of its repository (docs/design/rust-domain-authority.md, "Demand"):
 * the one thing the page tells Rust of itself (`set_demand`, by the window's subscription), which the session asks its
 * facts by. A surface holds a part while it shows it (`showBranch`, `showStack`, `needBranch`, `previewBranch`,
 * `showReviews`, `showReviewWindowPullRequest`, `showRobotDetails`): each answers its release and is counted, so two surfaces of one branch hold it
 * once; a watched robot of no branch is held for the life of the window (`watchRobotInDemand`). The focus of the window
 * and whether the machine is online come with them.
 *
 * The review readiness: a surface that needs it now holds its subject (`demandReviewReadiness`, a branch or the
 * `stack:<root>` of a whole stack) in `needed`; a surface that shows the step of a subject holds it as a visible branch
 * or stack (`showReviewReadiness`), and the session asks those of them whose step waits for it
 * (`domain_policy::readiness::wanted`).
 *
 * A change goes out at most four times a second (`DEMAND_SEND_INTERVAL_MS`), without waiting for an answer, and only
 * when it differs from what was sent; a new subscription (another repository, a session opened again) gets it again.
 * The session asks the robots, the review readiness and the details by it, and polls nothing while the machine is
 * offline (the review requests).
 */
import { useEffect } from "react"
import type { Demand } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { stackRootOfReadinessKey } from "@/lib/whole-stack"

/** The least time between two sends of the demand */
export const DEMAND_SEND_INTERVAL_MS = 250

type Part = "visibleBranches" | "visibleStacks" | "needed" | "robotDetails" | "watchedRobots" | "reviews" | "reviewWindows"

const PARTS: readonly Part[] = ["visibleBranches", "visibleStacks", "needed", "robotDetails", "watchedRobots", "reviews", "reviewWindows"]

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

/** The Reviews view shows until the returned function is called: the session reads the rest of Reviews */
export function showReviews(): () => void {
  return hold("reviews", "reviews")
}

/**
 * A review window shows the pull request `number` until the returned function is called: its actions stay in the view
 * also once no listing has it
 */
export function showReviewWindowPullRequest(number: number): () => void {
  return hold("reviewWindows", String(number))
}

/** A surface needs the readiness of the subject now (the Safe Push popover and its click, the "Create pull request" dialog) until the returned function is called */
export function demandReviewReadiness(subject: string): () => void {
  return hold("needed", subject)
}

/** `demandReviewReadiness` while `enabled` */
export function useDemandReviewReadiness(subject: string, enabled: boolean): void {
  useEffect(() => (enabled ? demandReviewReadiness(subject) : undefined), [subject, enabled])
}

/**
 * A surface shows the next step of the subject until the returned function is called (a row, an island header, the
 * preview, the review window): a branch, or the `stack:<root>` of a whole stack, held as a visible branch or stack; the
 * session asks for its readiness while its step depends on it
 */
export function showReviewReadiness(subject: string): () => void {
  const root = stackRootOfReadinessKey(subject)
  return root === null ? hold("visibleBranches", subject) : hold("visibleStacks", root)
}

/** `showReviewReadiness` while the component shows the step of `subject`; nothing for an empty one */
export function useShowReviewReadiness(subject: string): void {
  useEffect(() => (subject === "" ? undefined : showReviewReadiness(subject)), [subject])
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
  const names = (part: Part, also: readonly string[] = []) => [...new Set([...held.get(part)!.keys(), ...also])].sort()
  return {
    focused,
    online: typeof navigator === "undefined" || navigator.onLine,
    visibleBranches: names("visibleBranches"),
    visibleStacks: names("visibleStacks"),
    needed: names("needed"),
    robotDetails: names("robotDetails"),
    watchedRobots: names("watchedRobots"),
    reviews: held.get("reviews")!.size > 0,
    reviewWindows: names("reviewWindows")
      .map(Number)
      .toSorted((a, b) => a - b),
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
