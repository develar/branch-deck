import { describe, expect, it, vi } from "vite-plus/test"
import { loadWindowPage } from "@/router"

vi.mock("@/pages/ActivityPage", () => ({ ActivityPage: () => null }))

describe("loadWindowPage", () => {
  it("loads the page of the path of the window", async () => {
    const { ActivityPage } = await import("@/pages/ActivityPage")
    await expect(loadWindowPage("/activity")).resolves.toBe(ActivityPage)
  })

  it("refuses a path that has no page, a name of an object's prototype too", async () => {
    await expect(loadWindowPage("/nothing")).rejects.toThrow("No page at /nothing")
    await expect(loadWindowPage("/settings/")).rejects.toThrow("No page at /settings/")
    await expect(loadWindowPage("constructor")).rejects.toThrow("No page at constructor")
  })
})
