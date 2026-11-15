import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { ApiError, HideLandedParams, LandedBranch } from "@/lib/bindings"
import { hideAfterSync, ISLAND_MIN_SEEN_MS, landedStore, markLandedSeen, setLandedStatus } from "@/stores/landed"

const { backend } = vi.hoisted(() => ({
  backend: {
    /** The hides asked, in order */
    hides: [] as HideLandedParams[],
    /** The answer of the next hide: its error, or none */
    refusal: null as ApiError | null,
    /** The answer of the last hide: awaited after the page's own handler of it, which it took first */
    answered: Promise.resolve() as Promise<unknown>,
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    hideLanded: (params: HideLandedParams) => {
      backend.hides.push(params)
      const refusal = backend.refusal
      backend.refusal = null
      const answer = Promise.resolve(refusal === null ? { status: "ok", data: null } : { status: "error", error: refusal })
      backend.answered = answer
      return answer
    },
  },
}))
vi.mock("@/stores/activity", () => ({ recordLanded: () => {} }))

const REPOSITORY = "/repo"
const branch: LandedBranch = { name: "auth", commits: [], remaining: 0, landedAt: 1_700_000_000 }

function refusal(code: ApiError["code"]): ApiError {
  return { code, message: "refused", details: null, retry: "after_reread", operation_id: null }
}

describe("the hide of the Landed island after a sync", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    backend.hides = []
    backend.refusal = null
    landedStore.setState({ repositoryPath: null, landing: [], landed: [], upstream: null, seenAt: null })
    setLandedStatus(REPOSITORY, [], [branch], "origin/master")
    markLandedSeen(REPOSITORY, branch.landedAt)
    vi.advanceTimersByTime(ISLAND_MIN_SEEN_MS)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("asks again on the view of the lease read when the session knew no lease of its slot, and only then", async () => {
    // the session of the view of epoch 1 knew no lease of its slot: nothing is hidden, and it opens anew under the lease
    backend.refusal = refusal("lease_read_again")
    hideAfterSync(REPOSITORY, 1)
    await backend.answered
    expect(landedStore.getState().seenAt).not.toBeNull()
    // the next sync, on the view of that lease, asks again for what was seen, at once
    hideAfterSync(REPOSITORY, 2)
    await backend.answered
    expect(backend.hides).toEqual([
      { repositoryPath: REPOSITORY, landedAt: branch.landedAt, epoch: 1 },
      { repositoryPath: REPOSITORY, landedAt: branch.landedAt, epoch: 2 },
    ])
    // that one hid it: a later sync asks nothing more
    expect(landedStore.getState().seenAt).toBeNull()
    hideAfterSync(REPOSITORY, 2)
    expect(backend.hides).toHaveLength(2)
  })

  it("keeps the island unseen after another refusal, as a hide of the view of another lease is", async () => {
    backend.refusal = refusal("internal")
    hideAfterSync(REPOSITORY, 1)
    await backend.answered
    // the answer came, and the island is not seen again by it: the next sync asks nothing
    expect(backend.hides).toHaveLength(1)
    expect(landedStore.getState().seenAt).toBeNull()
    hideAfterSync(REPOSITORY, 1)
    expect(backend.hides).toHaveLength(1)
  })
})
