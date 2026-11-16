import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot, captureModalSnapshot } from "../helpers/aria-snapshot-helpers"
import { boxOf } from "../helpers/header-helpers"
import { openModal } from "../helpers/modal-helpers"
import { pressAndExpectSettingsOpened } from "../helpers/settings-helpers"
import { emitTauriEvent, runAndWaitForAnswer } from "../helpers/sync-helpers"
import { TestRepositoryBuilder } from "../helpers/test-repository"
import { useTitleBar } from "../helpers/title-bar-helpers"
import { expectClosed, holdCommand, routeCommandError } from "../helpers/wait-helpers"
import { tauriMockInitScript } from "../setup/tauri-mock-script"

async function focusRepositoryStep(page: Page) {
  const step = page.getByRole("button", { name: "Select a Git Repository…" })
  await step.focus()
  await expect(step).toBeFocused()
  return step
}

test.describe("Welcome Card", () => {
  test("shows the welcome card for new users, which asks for the repository first", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", {
      prepopulateStore: false, // Don't auto-populate store so welcome card shows
    })

    // Verify the welcome card structure using HTML snapshot
    const welcomeCard = page.locator('[data-testid="welcome-card"]')
    await expect(welcomeCard).toBeVisible()
    await expect(welcomeCard.getByTestId("welcome-step-number")).toHaveText(["1", "2"])
    const steps = welcomeCard.getByRole("listitem")
    await expect(steps).toHaveCount(2)
    await expect(steps.nth(0)).toHaveAttribute("aria-current", "step")
    await expect(steps.nth(1)).toContainText("After selecting a repository, set a personal prefix if needed.")
    await expect(steps.nth(1).getByRole("button")).toHaveCount(0)
    await expect(welcomeCard.getByRole("button", { name: "Select a Git Repository…" })).toBeVisible()
    await expect(welcomeCard.getByRole("textbox", { name: "Branch prefix to save" })).toHaveCount(0)

    await captureHtmlSnapshot(welcomeCard, "welcome-card-new-user")
  })

  for (const theme of ["light", "dark"] as const) {
    test(`keeps onboarding columns aligned through selection in ${theme} theme`, { tag: "@webkit" }, async ({ page, setupRepo }) => {
      await page.emulateMedia({ colorScheme: theme })
      await page.setViewportSize({ width: 540, height: 800 })
      const repo = await setupRepo("simple_no_prefix", { prepopulateStore: false })
      const card = page.getByTestId("welcome-card")
      const numbers = card.getByTestId("welcome-step-number")
      const prefixTitle = card.getByRole("heading", { name: "Choose your branch prefix" })
      const select = card.getByRole("button", { name: "Select a Git Repository…" })
      const before = await boxOf(numbers.nth(0))
      const titleX = (await boxOf(select.getByText("Select a Git Repository…", { exact: true }))).x
      expect((await boxOf(numbers.nth(1))).x).toBe(before.x)
      expect((await boxOf(prefixTitle)).x).toBe(titleX)
      const upcomingColor = await prefixTitle.evaluate(element => getComputedStyle(element).color)
      await expect.poll(() => numbers.nth(1).evaluate(element => getComputedStyle(element).color)).toBe(upcomingColor)
      const activeColor = await numbers.nth(0).evaluate(element => getComputedStyle(element).color)
      await expect.poll(() => select.getByText("Select a Git Repository…", { exact: true }).evaluate(element => getComputedStyle(element).color)).toBe(activeColor)
      const footer = card.getByText("Once your repository and branch prefix are ready, you can sync and start organizing your commits.")
      await expect(footer).toBeVisible()

      await select.click()
      const input = card.getByRole("textbox", { name: "Branch prefix to save" })
      await expect(input).toBeFocused()
      await expect(numbers).toHaveText(["1", "2"])
      const after = await boxOf(numbers.nth(0))
      expect(after.x).toBe(before.x)
      expect(after.y).toBe(before.y)
      expect((await boxOf(numbers.nth(1))).x).toBe(before.x)
      expect((await boxOf(card.getByRole("heading", { name: "Repository selected" }))).x).toBe(titleX)
      expect((await boxOf(prefixTitle)).x).toBe(titleX)
      await expect.poll(() => prefixTitle.evaluate(element => getComputedStyle(element).color)).not.toBe(upcomingColor)
      await expect.poll(() => prefixTitle.evaluate(element => getComputedStyle(element).color)).toBe(activeColor)
      await expect.poll(() => numbers.nth(1).evaluate(element => getComputedStyle(element).color)).toBe(activeColor)
      await expect.poll(() => card.getByRole("heading", { name: "Repository selected" }).evaluate(element => getComputedStyle(element).color)).not.toBe(activeColor)
      await expect(card.getByText("Step 1 complete", { exact: true })).toBeVisible()
      const path = card.getByText(repo.path!, { exact: true })
      expect((await boxOf(path)).x).toBe(titleX)
      await expect.poll(() => path.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
      await expect(footer).toBeVisible()
      await input.fill("aligned-user")
      await card.getByRole("button", { name: "Save", exact: true }).click()
      await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
    })
  }

  test("provides borderless repository action feedback and skips the upcoming step", { tag: "@webkit" }, async ({ page, setupRepo }) => {
    const repo = await setupRepo("NO_REPO", { prepopulateStore: false })
    await repo.setDialogPick(null)
    const select = page.getByRole("button", { name: "Select a Git Repository…" })
    const feedback = () =>
      select.evaluate(element => {
        const style = getComputedStyle(element)
        return { background: style.backgroundColor, border: style.borderTopWidth, cursor: style.cursor, translate: style.translate, outline: style.outlineStyle }
      })
    await page.mouse.move(0, 0)
    const resting = await feedback()
    expect(resting.border).toBe("0px")
    expect(resting.cursor).toBe("pointer")
    await select.hover()
    await expect.poll(async () => (await feedback()).background).not.toBe(resting.background)
    await page.mouse.down()
    await expect.poll(async () => (await feedback()).translate).not.toBe(resting.translate)
    await runAndWaitForAnswer(page, "browse_repository", () => page.mouse.up())
    await expect(select).toBeVisible()
    // Enter keyboard modality before focusing the action; do not depend on the host's reverse tab order.
    await page.keyboard.press("Tab")
    await select.focus()
    await expect(select).toBeFocused()
    await expect.poll(async () => (await feedback()).outline).toBe("solid")
    await page.keyboard.press("Tab")
    await expect.poll(() => page.getByTestId("welcome-card").evaluate(element => element.contains(document.activeElement))).toBe(false)
  })

  for (const activation of ["click", "Enter", "Space"] as const) {
    test(`selects a repository from the welcome step by ${activation}`, { tag: "@webkit" }, async ({ page, setupRepo }) => {
      await setupRepo("simple", { prepopulateStore: false })
      const step = page.getByRole("button", { name: "Select a Git Repository…" })
      await expect(step).toHaveAccessibleDescription("Browse to a local Git repository.")
      if (activation === "click") {
        // The description is part of the target, as the title and the number are.
        await step.getByText("Browse to a local Git repository.").click()
      } else {
        await step.focus()
        await expect(step).toBeFocused()
        await step.press(activation)
      }
      await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
      await expect(page.getByTestId("welcome-card")).toHaveCount(0)
    })
  }

  test("keeps the welcome step and its focus when the folder dialog is canceled", { tag: "@webkit" }, async ({ page, setupRepo }) => {
    const repo = await setupRepo("NO_REPO", { prepopulateStore: false })
    await repo.setDialogPick(null)
    const step = page.getByRole("button", { name: "Select a Git Repository…" })
    await step.focus()
    await expect(step).toBeFocused()
    await runAndWaitForAnswer(page, "browse_repository", () => step.press("Enter"))
    await expect(step).toBeVisible()
    await expect(step).toBeFocused()
    await expect(page.getByRole("combobox")).toContainText("Select repository…")
    await expect(page.getByTestId("error-alert")).toHaveCount(0)
  })

  test("reports a non-git folder selected from the welcome step", async ({ page, setupRepo }) => {
    await setupRepo("empty-non-git", { prepopulateStore: false })
    await page.getByRole("button", { name: "Select a Git Repository…" }).click()
    await expect(page.getByTestId("error-alert")).toContainText("Not a Git repository")
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)
  })

  test("reports a folder dialog failure from the welcome step", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })
    await routeCommandError(page, "browse_repository", { code: "internal", message: "Folder dialog unavailable" })
    await page.getByRole("button", { name: "Select a Git Repository…" }).click()
    await expect(page.getByTestId("error-alert")).toContainText("Cannot browse for a repository")
    await expect(page.getByTestId("error-alert")).toContainText("Folder dialog unavailable")
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)
  })

  test("directs users to the repository selector when the host has no folder dialog", async ({ page, setupRepo }) => {
    await useTitleBar(page, { kind: "native", checkForUpdates: false, folderDialog: false })
    await setupRepo("NO_REPO", { prepopulateStore: false })
    const card = page.getByTestId("welcome-card")
    await expect(card.getByRole("heading", { name: "Select a Git repository" })).toBeVisible()
    await expect(card.getByText("Choose a repository in the repository selector above.")).toBeVisible()
    await expect(card.getByRole("heading", { name: "Choose your branch prefix" })).toBeVisible()
    await expect(card.getByTestId("welcome-step-number")).toHaveText(["1", "2"])
    await expect(card.getByRole("button", { name: "Select a Git Repository…" })).toHaveCount(0)
    await expect(page.getByTestId("browse-repository-button")).toHaveCount(0)
    const numbers = card.getByTestId("welcome-step-number")
    expect((await boxOf(numbers.nth(0))).x).toBe((await boxOf(numbers.nth(1))).x)
    expect((await boxOf(card.getByRole("heading", { name: "Select a Git repository" }))).x).toBe((await boxOf(card.getByRole("heading", { name: "Choose your branch prefix" }))).x)
    const activeColor = await numbers.nth(0).evaluate(element => getComputedStyle(element).color)
    await expect.poll(() => card.getByRole("heading", { name: "Select a Git repository" }).evaluate(element => getComputedStyle(element).color)).toBe(activeColor)
  })

  test("opens branch prefix help modal from welcome card", async ({ page, setupRepo }) => {
    // a repository without a prefix: the welcome card asks for one
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    // Wait for welcome card to be visible
    const welcomeCard = page.locator('[data-testid="welcome-card"]')
    await expect(welcomeCard).toBeVisible()

    // Click the "View guide" button to open modal
    const viewGuideButton = page.getByRole("button", { name: "View guide" })
    await openModal(page, viewGuideButton)

    // Verify the modal structure using HTML snapshot
    await captureModalSnapshot(page, "branch-prefix-help-modal")
  })

  test("opens Settings at the field of the prefix from the guide, and at the one of the repository", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })
    await page.getByRole("button", { name: "View guide" }).click()
    const guide = page.getByTestId("branch-prefix-help-dialog")
    await pressAndExpectSettingsOpened(page, () => guide.getByRole("button", { name: "Open Settings" }).click(), { page: "branches", focus: "branchPrefix" })
    await expectClosed(guide)

    // the prefix of one repository: its page in Settings
    await page.getByRole("button", { name: "View guide" }).click()
    await pressAndExpectSettingsOpened(
      page,
      () => guide.getByRole("button", { name: "Open Repository Settings" }).click(),
      { page: "branches", scope: "repository", focus: "branchPrefix" },
      2,
    )
    await expectClosed(guide)
  })

  test("shows the prefix step as step 1 when the repository has no branch prefix configured", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    const welcomeCard = page.locator('[data-testid="welcome-card"]')
    await expect(welcomeCard).toBeVisible()

    // Should show the prefix step (check for the heading specifically), numbered 1: the steps shown count from 1
    await expect(welcomeCard.getByRole("heading", { name: "Choose your branch prefix" })).toBeVisible()
    await expect(welcomeCard.getByTestId("welcome-step-number")).toHaveText(["1"])
    const activeColor = await welcomeCard.getByTestId("welcome-step-number").evaluate(element => getComputedStyle(element).color)
    await expect.poll(() => welcomeCard.getByRole("heading", { name: "Choose your branch prefix" }).evaluate(element => getComputedStyle(element).color)).toBe(activeColor)

    // Should NOT show Step 1 content
    await expect(welcomeCard.getByRole("button", { name: "Select a Git Repository…" })).toHaveCount(0)

    // the suggestion of the repository's git identity (test@example.com) pre-fills the field
    await expect(welcomeCard.getByRole("textbox", { name: "Branch prefix to save" })).toHaveValue("test")

    // Capture HTML snapshot for regression safety
    await captureHtmlSnapshot(welcomeCard, "welcome-card-repo-no-prefix")
  })

  test("keeps Step 2 visible after selecting a repository without branch prefix", { tag: "@webkit" }, async ({ page, setupRepo, testStore }) => {
    const welcome = await setupRepo("NO_REPO", { prepopulateStore: false })
    // the repository the user picks in the folder dialog: one without a branch prefix
    const picked = new TestRepositoryBuilder().useTemplate("simple_no_prefix").withPrepopulateStore(false)
    await picked.init()
    try {
      await welcome.setDialogPick(picked.path)

      // Welcome card should be visible initially
      const welcomeCard = page.locator('[data-testid="welcome-card"]')
      await expect(welcomeCard).toBeVisible()

      await welcomeCard.getByRole("button", { name: "Select a Git Repository…" }).click()

      // After repository selection, Step 2 should still be present if no prefix is configured
      // Re-query the card to avoid staleness if DOM re-rendered
      const welcomeCardAfter = page.locator('[data-testid="welcome-card"]')
      await expect(welcomeCardAfter).toBeVisible()
      await expect(welcomeCardAfter.getByRole("heading", { name: "Choose your branch prefix" })).toBeVisible()

      await expect(welcomeCardAfter.getByTestId("welcome-step-number")).toHaveText(["1", "2"])
      const completed = welcomeCardAfter.getByRole("listitem").filter({ hasText: "Repository selected" })
      await expect(completed).toContainText("Step 1 complete")
      await expect(completed).toContainText(picked.path!)
      await page.setViewportSize({ width: 540, height: 800 })
      const path = completed.getByText(picked.path!, { exact: true })
      await expect.poll(() => path.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
      await expect(completed.getByRole("button")).toHaveCount(0)
      await expect(welcomeCardAfter.getByRole("listitem").filter({ hasText: "Choose your branch prefix" })).toHaveAttribute("aria-current", "step")
      await expect(welcomeCardAfter.getByRole("textbox", { name: "Branch prefix to save" })).toBeFocused()

      // the field shows the suggestion of the selected repository's git identity (test@example.com)
      await expect(welcomeCardAfter.getByRole("textbox", { name: "Branch prefix to save" })).toHaveValue("test")

      // Capture HTML snapshot for regression safety
      await captureHtmlSnapshot(welcomeCardAfter, "welcome-card-repo-selected-no-prefix")
      await expect.poll(async () => ((await testStore.get("appSettings")) as { onboardingStatus?: string })?.onboardingStatus).toBe("inProgress")
      await page.reload()
      const prefix = page.getByRole("textbox", { name: "Branch prefix to save" })
      await expect(prefix).toBeVisible()
      await expect(prefix).not.toBeFocused()
      await expect(page.getByTestId("welcome-step-number")).toHaveText(["1", "2"])
      // in the card: the tooltip of the header's path has the same words as its description
      await expect(page.getByTestId("welcome-card").getByText(picked.path!, { exact: true })).toBeVisible()
      await prefix.fill("invalid prefix")
      await expect(prefix).toHaveAttribute("aria-invalid", "true")
      await expect(welcomeCardAfter.getByRole("button", { name: "Save", exact: true })).toBeDisabled()
      await prefix.fill("first-user")
      await prefix.press("Enter")
      await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
      await expect(welcomeCardAfter).toHaveCount(0)
      await expect.poll(async () => ((await testStore.get("appSettings")) as { onboardingStatus?: string })?.onboardingStatus).toBe("complete")
      await expect(page.getByTestId("ready-to-sync-card").getByTestId("agents-hint")).toBeVisible()
      await page.reload()
      await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
      await expect(page.getByTestId("welcome-card")).toHaveCount(0)
    } finally {
      // the page first, so it starts no command for the picked repository
      await page.close()
      await picked.cleanup()
    }
  })

  for (const activation of ["Enter", "Space"] as const) {
    test(`focuses prefix step 2 after repository selection by ${activation}`, { tag: "@webkit" }, async ({ page, setupRepo }) => {
      await setupRepo("simple_no_prefix", { prepopulateStore: false })
      const select = await focusRepositoryStep(page)
      await select.press(activation)
      await expect(page.getByRole("textbox", { name: "Branch prefix to save" })).toBeFocused()
      await expect(page.getByTestId("welcome-step-number")).toHaveText(["1", "2"])
    })
  }

  test("focuses prefix step 2 after the native picker returns focus to the page", { tag: "@webkit" }, async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: false })
    const select = await focusRepositoryStep(page)
    // A native sheet can answer while the webview is still inactive; browser dialog mocks do not model that interval.
    await page.evaluate(() => Object.defineProperty(document, "hasFocus", { configurable: true, value: () => false }))
    await select.press("Enter")
    const field = page.getByRole("textbox", { name: "Branch prefix to save" })
    await expect(field).toBeVisible()
    await expect(field).not.toBeFocused()
    await page.evaluate(() => {
      Reflect.deleteProperty(document, "hasFocus")
      window.dispatchEvent(new FocusEvent("focus"))
    })
    await expect(field).toBeFocused()
    // The main window also retries a refused subscription on native focus, replacing the prefix form.
    await runAndWaitForAnswer(page, "subscribe_view", () => emitTauriEvent(page, "tauri://focus"))
    await expect(field).toBeFocused()
    await page.keyboard.press("ControlOrMeta+A")
    await page.keyboard.type("after-picker")
    await expect(field).toHaveValue("after-picker")
  })

  test("keeps the completed repository visible while checking its prefix", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: false })
    const release = await holdCommand(page, "subscribe_view")
    try {
      await page.getByRole("button", { name: "Select a Git Repository…" }).click()
      const card = page.getByTestId("welcome-card")
      await expect(card.getByText("Repository selected", { exact: true })).toBeVisible()
      await expect(card.getByText(repo.path!, { exact: true })).toBeVisible()
      await expect(card.getByRole("status")).toHaveText("Checking branch prefix…")
      await expect(card.getByTestId("welcome-step-number")).toHaveText(["1", "2"])
      const activeColor = await card
        .getByTestId("welcome-step-number")
        .nth(1)
        .evaluate(element => getComputedStyle(element).color)
      await expect.poll(() => card.getByRole("heading", { name: "Choose your branch prefix" }).evaluate(element => getComputedStyle(element).color)).toBe(activeColor)
      await expect(card.getByRole("textbox")).toHaveCount(0)
      await expect(page.getByTestId("ready-to-sync-card")).toHaveCount(0)
    } finally {
      release()
    }
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
  })

  test("resumes first-time progress from saved settings in a fresh window without taking focus", { tag: "@webkit" }, async ({ page, setupRepo, testStore }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: false })
    await page.getByRole("button", { name: "Select a Git Repository…" }).click()
    await expect(page.getByRole("textbox", { name: "Branch prefix to save" })).toBeFocused()
    await expect.poll(async () => ((await testStore.get("appSettings")) as { onboardingStatus?: string })?.onboardingStatus).toBe("inProgress")
    const saved = { appSettings: await testStore.get("appSettings"), recentProjects: await testStore.get("recentProjects") }
    const url = page.url()
    await page.close()
    const restarted = await page.context().newPage()
    await restarted.addInitScript(tauriMockInitScript())
    await restarted.addInitScript(store => Object.assign(window, { __TAURI_STORE__: store, __TAURI_STORE_VERSION__: 0 }), saved)
    await restarted.goto(url)
    const card = restarted.getByTestId("welcome-card")
    await expect(card.getByText("Step 1 complete")).toBeVisible()
    await expect(card.getByTestId("welcome-step-number")).toHaveText(["1", "2"])
    const field = card.getByRole("textbox", { name: "Branch prefix to save" })
    await expect(field).toBeVisible()
    await expect(field).not.toBeFocused()
    await field.fill("fresh-user")
    await card.getByRole("button", { name: "Save", exact: true }).click()
    await expect(restarted.getByTestId("ready-to-sync-card")).toBeVisible()
  })

  test("retains first-time progress after an invalid folder and a successful retry", async ({ page, setupRepo, otherRepository }) => {
    const repo = await setupRepo("empty-non-git", { prepopulateStore: false })
    await page.getByRole("button", { name: "Select a Git Repository…" }).click()
    await expect(page.getByTestId("error-alert")).toContainText("Not a Git repository")
    await repo.setDialogPick(otherRepository)
    await page.getByTestId("browse-repository-button").click()
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
    await expect(page.getByTestId("error-alert")).toHaveCount(0)
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)
  })

  test("updates the completed repository row when switching before prefix setup", async ({ page, setupRepo }) => {
    const first = await setupRepo("simple_no_prefix", { prepopulateStore: false })
    const second = new TestRepositoryBuilder().useTemplate("simple_no_prefix").withPrepopulateStore(false)
    await second.init()
    try {
      await page.getByRole("button", { name: "Select a Git Repository…" }).click()
      const card = page.getByTestId("welcome-card")
      await expect(card.getByText(first.path!, { exact: true })).toBeVisible()
      await first.setDialogPick(second.path)
      await page.getByTestId("browse-repository-button").click()
      await expect(card.getByText(second.path!, { exact: true })).toBeVisible()
      await expect(card.getByText(first.path!, { exact: true })).toHaveCount(0)
      await expect(card.getByTestId("welcome-step-number")).toHaveText(["1", "2"])
      await expect(card.getByRole("textbox", { name: "Branch prefix to save" })).toHaveValue("test")
    } finally {
      await page.close()
      await second.cleanup()
    }
  })
})
