import { useEffect, useRef, useState } from "react"
import type { ReactNode, RefObject } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useStore } from "zustand"
import { PullRequestReview } from "@/components/review/PullRequestReview"
import type { PullRequestReviewHost, ReviewStackLayerView } from "@/components/review/PullRequestReview"
import { useRangeReadings } from "@/hooks/useRangeReadings"
import { useWindowTitle } from "@/hooks/useWindowTitle"
import type { PullRequestReviewEvent, ReadingRange, RequestRow, ReviewRange, ReviewRequest, ReviewsView } from "@/lib/bindings"
import { logError, notifyError } from "@/lib/errors"
import { openPullRequestUrl } from "@/lib/pull-request"
import type { ReviewFeedback } from "@/lib/pull-request-review"
import { showReviewToast } from "@/lib/pull-request-review"
import { ALL_CHANGES, layerOfItem, layerScopeKey } from "@/lib/range-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import type { ReviewPlace, ShownReviewWindow } from "@/lib/review-reuse"
import { reviewWindowTitle } from "@/lib/review-reuse"
import { askReviewRun, PULL_REQUEST_READ, PULL_REQUEST_SAFE_PUSH } from "@/lib/review-runs"
import { showReviewWindowPullRequest } from "@/model/demand"
import { useRowPart } from "@/model/store"
import { useBranchSync } from "@/stores/branch-sync"
import { reviewFetchKey, reviewStackFetchKey, startReviewFetch, startReviewStackFetch, stopReviewFetch, useReviewFetch } from "@/stores/review-fetch"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"
import { getReviewBusy, reviewRequestsStore, submitReview } from "@/stores/review-requests"
import { reviewRowsOf, reviewsStore } from "@/stores/reviews"
import type { ReviewRows } from "@/stores/reviews"

/*
 * The review of a pull request in the review window (openspec/specs/repository/review-requests/spec.md#review-window), and
 * of a stack of them (Review Stack), a chunk of its own: the page loads it for a pull request only, so the first frame
 * of the review of a branch or a commit keeps its size. It reads the requests, Reviews, the actions Rust decides and the
 * writes that run from the window's own session, fetches what it shows, approves and requests changes itself; the Safe
 * Push and the read mark of Your Turn are the main window's (`review/run`).
 */

type ShownPullRequest = Extract<ShownReviewWindow, { kind: "pullRequest" }>
type ShownStack = Extract<ShownReviewWindow, { kind: "pullRequestStack" }>

interface Shown {
  readonly request: ReviewRequest
  /** Its place in the list when it was shown */
  readonly index: number
  readonly intent: ReviewIntent | null
  /** A new value per shown request: its intent is offered once */
  readonly token: number
}

const NO_REQUESTS: readonly ReviewRequest[] = []
const NO_GROUPS: PullRequestReviewHost["reviewGroups"] = []

/** The rows of the list the window walks: Your Turn's, or those of Reviews with how the user takes part in each */
function useListRows(repositoryPath: string, source: ShownPullRequest["source"]): ReviewRows {
  const requests = useStore(reviewRequestsStore, state => state.repositories[repositoryPath]?.requests ?? NO_REQUESTS)
  const sections = useStore(reviewsStore, state => state.repositories[repositoryPath]?.sections ?? NO_SECTIONS)
  return source === "yourTurn" ? { requests, involvements: NO_INVOLVEMENTS } : reviewRowsOf(requests, sections)
}

const NO_SECTIONS: ReviewsView["sections"] = []
const NO_INVOLVEMENTS: ReviewRows["involvements"] = {}

/** How a write the window runs ends: its toast and its error in the window */
const WINDOW_REVIEW_FEEDBACK: ReviewFeedback = { toast: showReviewToast, error: notifyError }

/**
 * The review of the pull request of `data`: ↑/↓ walk the list it came from, a move (an open of a request of that list)
 * shows that request; Back restores the request it showed (`onPlace`)
 */
export function PullRequestReviewWindow({ data, back, onPlace }: { data: ShownPullRequest; back: ReactNode; onPlace: (place: ReviewPlace) => void }) {
  const { repositoryPath, source } = data
  const [shown, setShown] = useState<Shown>({ request: data.request, index: data.index, intent: data.intent, token: data.openedAt })
  // a move: an open of a request of the list on screen shows it
  const [openedAt, setOpenedAt] = useState(data.openedAt)
  if (data.openedAt !== openedAt) {
    setOpenedAt(data.openedAt)
    if (shown.request.number !== data.number) {
      setShown({ request: data.request, index: data.index, intent: null, token: data.openedAt })
    }
  }
  const rows = useListRows(repositoryPath, source)
  const bodyRef = useRef<HTMLDivElement>(null)
  const [selected, setSelected] = useState(ALL_CHANGES)

  const live = rows.requests.find(it => it.number === shown.request.number)
  const request = live ?? shown.request
  const index = live === undefined ? -1 : rows.requests.indexOf(live)
  const number = request.number
  const writes = useRequestWrites(repositoryPath, request)
  const fetch = useReviewFetch(reviewFetchKey(repositoryPath, request))
  useEffect(() => {
    startReviewFetch(repositoryPath, request)
  }, [repositoryPath, request])
  // what the fetched pull request leaves to read, with its author's word checked against it
  const fetchedRange = fetch?.status === "fetched" ? fetch.range : null
  const readings = useRangeReadings(repositoryPath, fetchedRange === null ? null : [readingRange(fetchedRange, request)])

  // the session keeps the review refs and the actions of the request the window shows, and asks Patronus of it
  useEffect(() => showReviewWindowPullRequest(number), [number])
  useWindowStart(bodyRef)
  useEffect(() => {
    onPlace({ kind: "pullRequest", request, index: index === -1 ? shown.index : index })
  })

  const host: PullRequestReviewHost = {
    ...writes,
    source,
    requests: rows.requests,
    involvement: rows.involvements[number] ?? null,
    index,
    openedIndex: shown.index,
    withdrawn: live === undefined && writes.submitted === undefined,
    intent: shown.intent,
    openToken: shown.token,
    openedAt,
    fetch,
    fetchAgain: () => startReviewFetch(repositoryPath, request),
    reading: readings?.readings[0] ?? null,
    selected,
    onSelect: setSelected,
    showRequest: next => {
      const target = rows.requests[next]
      if (target === undefined) {
        return
      }
      setShown({ request: target, index: next, intent: null, token: Date.now() })
      markRead(repositoryPath, target)
    },
  }

  // the native title names the request shown
  useWindowTitle(reviewWindowTitle({ ...data, number, request }))
  return <PullRequestReview host={host} bodyRef={bodyRef} back={back} />
}

/**
 * The review of the stack of `data` (Review Stack): its layers as its fetch answers them (Rust chains them from its top
 * down to its bottom; the ones it opened with until then), each one's request as Your Turn has it now; the item of the
 * column decides the layer whose decisions the title bar holds, ↑/↓ walk the layers. A push, a layer in between or a
 * merged one is another fetch: its key holds every layer the window knows and every layer of the row of Your Turn
 * the top is in
 */
export function PullRequestStackReviewWindow({ data, back }: { data: ShownStack; back: ReactNode }) {
  const { repositoryPath, top, bottom } = data
  const requests = useStore(reviewRequestsStore, state => state.repositories[repositoryPath]?.requests ?? NO_REQUESTS)
  const rows = useStore(reviewRequestsStore, state => state.repositories[repositoryPath]?.rows ?? NO_ROWS)
  const bodyRef = useRef<HTMLDivElement>(null)
  const [selected, setSelected] = useState(ALL_CHANGES)
  // the user chose an item of the column, a layer or All Changes: the window no longer opens on the first layer to read
  const [chosen, setChosen] = useState(false)
  // the layers of the last answer: the window shows them while the next fetch runs
  const [answered, setAnswered] = useState<readonly ReviewStackLayerView[] | null>(null)

  const known = answered?.map(layer => layer.request) ?? data.layers
  const liveOf = (number: number) => requests.find(it => it.number === number) ?? known.find(it => it.number === number)
  const row = rows.find(it => it.kind === "stack" && it.layers.includes(top))
  const numbers = [...new Set([...known.map(it => it.number), ...(row?.kind === "stack" ? row.layers : [])])]
  const key = reviewStackFetchKey(
    repositoryPath,
    top,
    bottom,
    numbers.flatMap(number => liveOf(number) ?? []),
  )
  const fetch = useReviewFetch(key)
  const fetchedLayers = fetch?.status === "fetched" ? (fetch.layers ?? null) : null
  if (fetchedLayers !== null && fetchedLayers !== answered) {
    setAnswered(fetchedLayers)
  }
  const layers: readonly ReviewStackLayerView[] = (fetchedLayers ?? answered ?? data.layers.map(request => ({ request, range: null, restacked: false }))).map(layer => ({
    request: requests.find(it => it.number === layer.request.number) ?? layer.request,
    range: fetchedLayers === null ? null : layer.range,
    restacked: layer.restacked,
  }))
  const layer = layerOfItem(
    layers.map(it => ({ request: it.request, commits: it.range?.commits ?? null, restacked: it.restacked })),
    selected,
  )
  const index = layers.findIndex(it => it.request.number === layer)
  const request = layers[index]?.request ?? layers.at(-1)?.request ?? data.layers.at(-1)!
  const writes = useRequestWrites(repositoryPath, request)
  // what each layer leaves to read, in one read of all of them once the stack is fetched
  const readingRanges = fetchedLayers === null ? null : layers.flatMap(it => (it.range === null ? [] : [readingRange(it.range, it.request)]))
  const readings = useRangeReadings(repositoryPath, readingRanges !== null && readingRanges.length === layers.length ? readingRanges : null)
  // the window opens on the first layer with something to read, as Rust tells it, unless the user chose an item first
  const firstToRead = readings?.stack.firstToRead ?? null
  const opensOn = firstToRead === null ? undefined : layers[firstToRead]
  if (!chosen && opensOn !== undefined && selected === ALL_CHANGES) {
    setChosen(true)
    setSelected(layerScopeKey(opensOn.request.number))
  }
  const choose = (key: string) => {
    setChosen(true)
    setSelected(key)
  }

  useEffect(() => {
    startReviewStackFetch(repositoryPath, key, top, bottom)
  }, [repositoryPath, key, top, bottom])
  // the session keeps the review refs, the actions and the readiness of every layer
  const shownNumbers = layers.map(it => it.request.number).join(",")
  useEffect(() => {
    const releases = shownNumbers.split(",").map(number => showReviewWindowPullRequest(Number(number)))
    return () => releases.forEach(release => release())
  }, [shownNumbers])
  useWindowStart(bodyRef)

  const host: PullRequestReviewHost = {
    ...writes,
    source: "yourTurn",
    requests: layers.map(it => it.request),
    involvement: null,
    index,
    // ↓ from All Changes shows the bottom
    openedIndex: 0,
    withdrawn: layer !== null && !requests.some(it => it.number === layer) && writes.submitted === undefined,
    intent: data.intent,
    openToken: data.openedAt,
    openedAt: data.openedAt,
    fetch,
    fetchAgain: () => startReviewStackFetch(repositoryPath, key, top, bottom),
    reading: index === -1 ? null : (readings?.readings[index] ?? null),
    selected,
    onSelect: choose,
    stack: {
      layers,
      layer,
      readings: readings?.readings ?? null,
      summary: readings?.stack ?? null,
      approveLayer: (target, commitId) => submitReview(repositoryPath, target, "approve", commitId, null, WINDOW_REVIEW_FEEDBACK),
    },
    showRequest: next => {
      const target = layers[next]
      if (target !== undefined) {
        choose(layerScopeKey(target.request.number))
      }
    },
  }
  useWindowTitle(reviewWindowTitle(data))
  return <PullRequestReview host={host} bodyRef={bodyRef} back={back} />
}

const NO_ROWS: readonly RequestRow[] = []

/** A fetched range to read, with the title and the labels of its pull request: the author's word */
function readingRange(range: ReviewRange, request: ReviewRequest): ReadingRange {
  return { base: range.base, head: range.head, title: request.title, labels: request.labels ?? [] }
}

/** Your Turn marks a request read once the user saw it: the main window keeps that mark */
function markRead(repositoryPath: string, target: ReviewRequest): void {
  void askReviewRun({ repositoryPath, subject: `pull:${target.number}`, itemId: PULL_REQUEST_READ, request: target })
}

/** The start of the window: the focus in the page, so the keys work from the start; its close stops the fetch */
function useWindowStart(bodyRef: RefObject<HTMLDivElement | null>): void {
  // the window leaves what it shows: its fetch stops, its range goes; a review of it again (Back) fetches it again
  useEffect(() => stopReviewFetch, [])
  useEffect(() => {
    bodyRef.current?.focus()
  }, [bodyRef])
}

/**
 * What the window shows and writes of `request`: the actions and reviewer groups Rust decides of it, what runs for it
 * and what the user submitted here until the session says it, and its writes (Approve, Request Changes, the Safe Push,
 * which the main window runs)
 */
function useRequestWrites(repositoryPath: string, request: ReviewRequest) {
  const number = request.number
  const [localBusy, setLocalBusy] = useState<Readonly<Record<number, ReviewAction>>>({})
  const [localReviewed, setLocalReviewed] = useState<Readonly<Record<number, SubmittedReview>>>({})
  const reviewed = useStore(reviewRequestsStore, state => state.repositories[repositoryPath]?.reviewed[number])
  const operations = useBranchSync(state => state.operations)
  const actions = useRowPart(`pull:${number}`, "actions")
  const reviewGroups = useRowPart(`pull:${number}`, "reviewGroups") ?? NO_GROUPS
  const approvalLanding = useRowPart(`pull:${number}`, "approvalLanding")
  const baselineBranch = useBranchSync(state => state.baselineBranch)

  const run = (action: ReviewAction, write: () => Promise<boolean>, submittedReview: SubmittedReview | null): Promise<boolean> => {
    setLocalBusy(previous => ({ ...previous, [number]: action }))
    return write()
      .then(ok => {
        if (ok && submittedReview !== null) {
          setLocalReviewed(previous => ({ ...previous, [number]: submittedReview }))
        }
        return ok
      })
      .finally(() => setLocalBusy(previous => Object.fromEntries(Object.entries(previous).filter(([key]) => key !== String(number)))))
  }
  const review = (event: PullRequestReviewEvent, commitId: string, body: string | null) =>
    run(event, () => submitReview(repositoryPath, request, event, commitId, body, WINDOW_REVIEW_FEEDBACK), { event, commitId })
  return {
    repositoryPath,
    request,
    submitted: localReviewed[number] ?? reviewed,
    busy: localBusy[number] ?? getReviewBusy(operations)[number],
    actions,
    reviewGroups,
    approvalLanding,
    baselineBranch,
    close: () => {
      getCurrentWindow()
        .close()
        .catch((error: unknown) => logError("Cannot close the review window", error))
    },
    openOnGitHub: () => {
      markRead(repositoryPath, request)
      openPullRequestUrl(request.url)
    },
    approve: (commitId: string) => review("approve", commitId, null),
    requestChanges: (commitId: string, body: string) => review("requestChanges", commitId, body),
    // Patronus answers the comment of the Safe Push in the main window, which watches the robot
    startSafePush: (commitId: string) =>
      run("safePush", () => askReviewRun({ repositoryPath, subject: `pull:${number}`, itemId: PULL_REQUEST_SAFE_PUSH, commitIds: [commitId], request }), null),
  }
}
