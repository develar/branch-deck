import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { LandedSeenParams } from "@/lib/bindings"
import { markLandedSeen } from "@/stores/landed"

const { backend } = vi.hoisted(() => ({
  backend: {
    /** The sightings told to the session, in order */
    seen: [] as LandedSeenParams[],
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    landedSeen: (params: LandedSeenParams) => {
      backend.seen.push(params)
      return Promise.resolve({ status: "ok", data: null })
    },
  },
}))
vi.mock("@/stores/activity", () => ({ recordLanded: () => {} }))

describe("the sighting of the Landed island", () => {
  beforeEach(() => {
    backend.seen = []
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it("tells the session what the island shows on which view while the window has focus, and nothing without it", () => {
    let focused = false
    vi.stubGlobal("document", { hasFocus: () => focused })
    markLandedSeen("/repo", 1_700_000_000, 2)
    expect(backend.seen).toEqual([])
    focused = true
    markLandedSeen("/repo", 1_700_000_000, 2)
    expect(backend.seen).toEqual([{ repositoryPath: "/repo", landedAt: 1_700_000_000, epoch: 2 }])
  })
})
