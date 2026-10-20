// @spec spec/repository/zoom.spec.md
import { describe, expect, it } from "vite-plus/test"
import { minSubWindowSize, minWindowWidth, nextZoom } from "@/lib/zoom"

describe("nextZoom", () => {
  it("steps through the levels", () => {
    expect(nextZoom(1, "in")).toBe(1.1)
    expect(nextZoom(1.1, "in")).toBe(1.25)
    expect(nextZoom(1, "out")).toBe(0.9)
    expect(nextZoom(0.75, "out")).toBe(0.67)
  })

  it("stops at the first and the last level", () => {
    expect(nextZoom(1.5, "in")).toBe(1.5)
    expect(nextZoom(0.67, "out")).toBe(0.67)
  })

  it("steps from a level between the levels to the next one in that direction", () => {
    expect(nextZoom(1.2, "in")).toBe(1.25)
    expect(nextZoom(1.2, "out")).toBe(1.1)
    expect(nextZoom(3, "out")).toBe(1.5)
    expect(nextZoom(0.3, "in")).toBe(0.67)
  })

  it("resets to 100%", () => {
    expect(nextZoom(1.5, "reset")).toBe(1)
  })
})

describe("minWindowWidth", () => {
  it("keeps 768 CSS px for the islands, so a zoom above 125% widens the minimum window", () => {
    expect(minWindowWidth(0.67)).toBe(960)
    expect(minWindowWidth(1.25)).toBe(960)
    expect(minWindowWidth(1.5)).toBe(1152)
  })
})

describe("minSubWindowSize", () => {
  it("is the size of the title bar of a review up to 125%, wider above", () => {
    expect(minSubWindowSize("branch-review", 0.67)).toEqual({ width: 800, height: 500 })
    expect(minSubWindowSize("commit-review", 1.25)).toEqual({ width: 800, height: 500 })
    expect(minSubWindowSize("branch-review", 1.5)).toEqual({ width: 960, height: 500 })
  })

  it("is smaller for Settings, and wider above 107%", () => {
    expect(minSubWindowSize("settings", 1)).toEqual({ width: 600, height: 420 })
    expect(minSubWindowSize("settings", 1.25)).toEqual({ width: 700, height: 420 })
    expect(minSubWindowSize("settings", 1.5)).toEqual({ width: 840, height: 420 })
  })

  it("is smaller still for Activity, and wider above 110%", () => {
    expect(minSubWindowSize("activity", 1)).toEqual({ width: 440, height: 400 })
    expect(minSubWindowSize("activity", 1.25)).toEqual({ width: 500, height: 400 })
  })
})
