// @spec openspec/specs/branch/declared-needs/spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availableRobots, expectPatronusStarts, startRequest } from "../helpers/patronus-helpers"
import { openSafePushWithKey, startAnyway } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu, openSubmenu } from "../helpers/selection-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"

// the `issue_links` template: JIRA-456 and feature-auth are branches of their own, neither built on the other

/** The step of the row's slot */
function step(page: Page, branchName: string) {
  return findBranchRow(page, branchName).getByTestId("next-step-slot")
}

/** Add Need… of the menu of `branchName`, `needed` picked from the chips of the form and added with Enter */
async function addNeed(page: Page, branchName: string, needed: string) {
  await openContextMenu(page, findBranchRow(page, branchName))
  await clickContextMenuItem(page, "Add Need…")
  const form = page.getByTestId("inline-need-input")
  const field = form.getByRole("textbox")
  await expect(field).toBeFocused()
  await expect(form).toContainText(`What lands before ${branchName}`)
  await form.getByTestId("need-chips").getByRole("button", { name: needed }).click()
  await expect(field).toHaveValue(needed)
  await runAndWaitForSync(page, () => field.press("Enter"))
  await expect(form).toHaveCount(0)
}

test.describe("Declared needs", () => {
  // the focus of the form and the submenu: WebKit too
  test(
    "should declare from the menu what lands before a branch, name it on the row, and undo the declaration",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRepo("issue_links", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await expect(step(page, "JIRA-456")).toHaveAttribute("data-step", /./)
      await expect(step(page, "JIRA-456")).not.toHaveAttribute("data-step", "needsBranch")

      await addNeed(page, "JIRA-456", "feature-auth")
      const declared = page.locator("[data-slot=toast]", { hasText: "Declared that feature-auth lands before JIRA-456" })
      await expect(declared).toBeVisible()
      await runAndWaitForSync(page, () => declared.getByRole("button", { name: "Undo" }).click())
      await expect(step(page, "JIRA-456")).not.toHaveAttribute("data-step", "needsBranch")

      await addNeed(page, "JIRA-456", "feature-auth")
      // known as soon as the sync read the store
      await expect(step(page, "JIRA-456")).toHaveAttribute("data-step", "needsBranch")
      const button = step(page, "JIRA-456").getByTestId("next-step")
      await expect(button).toHaveText("Needs feature-auth")
      await button.hover()
      await expect(page.getByTestId("next-step-hover")).toHaveText(
        "You declared that feature-auth lands before JIRA-456: it is not in master yet, so a start of JIRA-456 lands them out of order.",
      )
      // the branch it needs needs nothing
      await expect(step(page, "feature-auth")).not.toHaveAttribute("data-step", "needsBranch")
    },
  )

  // the removal of the needs apart from their declaration: one test of both, six rewrites, took 30 s under load
  test("should take back a declared need from the menu, one of two from its submenu", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    await addNeed(page, "JIRA-456", "feature-auth")
    const button = step(page, "JIRA-456").getByTestId("next-step")
    await expect(button).toHaveText("Needs feature-auth")

    // a second need: Remove Need is a submenu of both
    await addNeed(page, "JIRA-456", "feature-api")
    await expect(button).toHaveText("Needs feature-api")
    await openContextMenu(page, findBranchRow(page, "JIRA-456"))
    await openSubmenu(page, "Remove Need")
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "feature-api"))
    await expect(button).toHaveText("Needs feature-auth")

    await openContextMenu(page, findBranchRow(page, "JIRA-456"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Remove Need: feature-auth"))
    await expect(step(page, "JIRA-456")).not.toHaveAttribute("data-step", "needsBranch")
  })

  test(
    "should say in the Safe Push popover what lands first, and start anyway with the override of the declared order",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("issue_links", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await addNeed(page, "JIRA-456", "feature-auth")
      await expect(step(page, "JIRA-456").getByTestId("next-step")).toHaveText("Needs feature-auth")

      const popover = await openSafePushWithKey(page, "JIRA-456")
      await expect(popover.getByTestId("safe-push-landing-needs")).toContainText(
        "You declared that feature-auth lands before JIRA-456: it is not in master yet, so a start of JIRA-456 lands them out of order.",
      )
      await startAnyway(page, "JIRA-456", { allowUnlanded: true })
      await expectPatronusStarts(repo, [startRequest("JIRA-456")])
    },
  )
})
