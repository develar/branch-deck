// @spec openspec/specs/repository/disk-usage/spec.md
import { appendFileSync, existsSync } from "node:fs"
import { dirname, join } from "node:path"
import { expect, test } from "../fixtures/test-fixtures"
import { openPalette, runByKeyboard } from "../helpers/command-palette-helpers"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"

/** What `brd-host disk --json` carries for a host: one pool of three slots, a limit of 2, one free slot past it */
const STUDIO_REPORT = {
  free_bytes: 150 * 1024 ** 3,
  min_free: 200 * 1024 ** 3,
  pools: [
    {
      git_common_dir: "/Users/me/projects/idea-1/.git",
      repository: "/Users/me/projects/idea-1",
      max_slots: 2,
      past_limit: 1,
      bytes: 50 * 1024 ** 3,
      slots: [
        { slot: "idea-11", path: "/Users/me/.branch-deck/pool/idea-1-0/1/idea-1", state: "ready", holder: "brd-host/1", bytes: 30 * 1024 ** 3, measured_at: 1, cached: true },
        { slot: "idea-12", path: "/Users/me/.branch-deck/pool/idea-1-0/2/idea-1", state: "free", bytes: 20 * 1024 ** 3, measured_at: 1, cached: true, trims_next: true },
        { slot: "idea-13", path: "/Users/me/.branch-deck/pool/idea-1-0/3/idea-1", state: "free", bytes: 0, measured_at: 1, cached: true },
      ],
    },
  ],
  bytes: 50 * 1024 ** 3,
  ms: 3,
}

test.describe("Disk usage", () => {
  test("should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    // three free slots: one past the limit of the repository's config, the one returned first. Each slot is a real get
    // and return (15 git processes): eleven past the default limit of 10 took half the test under load
    appendFileSync(join(repo.path, ".git", "config"), "[branchdeck]\n\tleaseMaxSlots = 2\n")
    const slots = await repo.addLeaseSlots(3)
    const [first, last] = [slots[0] ?? "", slots[2] ?? ""]
    await repo.setHostDisks([
      { alias: "studio", report: STUDIO_REPORT },
      { alias: "mini", error: "the agent on mini has no feature disk: `brd-host upgrade mini` sends it this build" },
    ])
    // the hosts answer once the test lets them: until then the walk of this Mac shows its progress, each slot counted
    await repo.holdRemoteAnswers({ hold: ["hostDisks"] })
    await openPalette(page)
    await runByKeyboard(page, "disk usage", "Show Disk Usage…")
    const dialog = page.getByTestId("disk-usage-dialog")
    const progress = dialog.getByRole("status").filter({ hasText: "Measured" })
    await expect(progress).toHaveText("Measured 3 of 3 slots")
    await expect.poll(() => repo.getArrivedAnswers()).toContain("hostDisks")
    await expect(dialog.getByRole("region", { name: "studio" })).toHaveCount(0)
    await repo.holdRemoteAnswers({ release: ["hostDisks"] })
    await expect(progress).toHaveCount(0)

    const thisMac = dialog.getByRole("region", { name: "This Mac" })
    const pool = thisMac.getByRole("group")
    await expect(pool.getByRole("listitem")).toHaveCount(3)
    await expect(pool.getByRole("listitem").first()).toContainText(/Measured (just now|[0-9]+ (minutes?|hours?|days?) ago)/)
    await expect(pool).toContainText("3 slots, limit 2, 1 goes with a trim")
    await expect(pool.getByRole("listitem").filter({ hasText: "Goes with a trim" })).toHaveCount(1)
    await expect(pool.getByRole("listitem").filter({ hasText: "Goes with a trim" })).toHaveAttribute("title", first)

    // a host with its pool, the slot it trims next and its free space below the minimum; a host that answered nothing says why
    const studio = dialog.getByRole("region", { name: "studio" })
    await expect(studio).toContainText("150.0 GiB free, a lease trims the pools below 200.0 GiB")
    await expect(studio.getByRole("group", { name: "idea-1" })).toContainText("50.0 GiB · 3 slots, limit 2, 1 goes with a trim")
    await expect(studio.getByRole("listitem").first()).toContainText("Leased by brd-host/1")
    await expect(dialog.getByRole("region", { name: "mini" })).toContainText("`brd-host upgrade mini` sends it this build")

    await expect(dialog).toContainText("Trim deletes 1 free slot on this Mac")
    // the trim deletes the slot, then the dialog reads the pools again: seconds under load
    await runAndWaitForAnswer(page, "trim_pools", () => dialog.getByRole("button", { name: "Trim to Limit" }).click())
    await expect(page.getByText("Trimmed 1 slot")).toBeVisible()
    // the slot returned first is gone with its number directory; the dialog measured again
    await expect(pool.getByRole("listitem")).toHaveCount(2)
    expect(existsSync(dirname(first))).toBe(false)
    expect(existsSync(last)).toBe(true)
    await expect(dialog.getByRole("button", { name: "Trim to Limit" })).toBeDisabled()
    await expect(dialog).toContainText("No free slot is past the limit")
  })
})
