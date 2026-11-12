// @spec openspec/specs/repository/ide-embedding/spec.md
import { test as base } from "@playwright/test"
import { IDE_THEMES, openFakeIde } from "../air-host/fake-host"
import type { FakeIde } from "../air-host/fake-host"
import type { HostContextTheme } from "../air-host/host-page"
import { expect } from "../fixtures/base-test"
import { TestRepositoryBuilder } from "../helpers/test-repository"
import { expectClosed } from "../helpers/wait-helpers"

/*
 * The Air build in a fake IDE (`tests/e2e/air-host/`): the frame of each view loads `dist-air/`, the host page relays the
 * frame link of MCP Apps with `x-air-app` to `brd mcp` of the test server (`mcp-host`) over its WebSocket. No mock of
 * Tauri: the page runs on its shim (`src/platform/air/`)
 */
const test = base.extend<{ ide: FakeIde; repo: TestRepositoryBuilder }>({
  // eslint-disable-next-line no-empty-pattern
  repo: async ({}, use) => {
    // `feature` with auth.js, and the unassigned "Fix critical bug" with bugfix.txt
    const repo = new TestRepositoryBuilder().useTemplate("single_unassigned")
    await repo.init()
    await use(repo)
    await repo.cleanup()
  },
  ide: async ({ page, repo }, use) => {
    await use(await openFakeIde(page, { repoId: repo.id, repositoryPath: repo.path }))
  },
})

/** Syncs the main view by its button and waits for the end of the sync: the row of the branch `feature`, settled */
async function syncedFeatureRow(ide: FakeIde) {
  const main = ide.frame("main:1")
  const sync = main.getByTestId("sync-button")
  await expect(sync).toBeEnabled()
  const before = Number(await sync.getAttribute("data-sync-count"))
  await sync.click()
  await expect(sync).not.toHaveAttribute("data-sync-count", String(before))
  await expect(sync).not.toHaveAttribute("aria-busy", "true")
  const row = main.locator('[data-testid="branch-row"][data-branch-name="feature"]')
  await expect(row).toBeVisible()
  return row
}

/** Opens the review of the branch `feature` from its menu in the main view: the frame of the review tab */
async function openFeatureReview(ide: FakeIde) {
  const row = await syncedFeatureRow(ide)
  await row.click({ button: "right" })
  await ide.frame("main:1").getByRole("menuitem", { name: "Review Changes" }).click()
  return ide.frame("review:1")
}

test.describe("Branch Deck in the IDE", () => {
  test("the tool window shows the branches of the repository after a sync of the page, each patch of the mirror a change of its rows", async ({ ide }) => {
    // the command ran through bd_invoke, and the patches of its run reached the view by the mirror
    await syncedFeatureRow(ide)
  })

  test("Review Changes opens the review as a view of the IDE, whose diffs stream as the call events of its call", async ({ ide }) => {
    const review = await openFeatureReview(ide)

    // brd asked the IDE for the view of the review (`x-air-app/openView`), and the IDE opened its place
    await expect.poll(async () => (await ide.views()).map(view => view.view)).toEqual(["main", "review"])
    const opened = (await ide.calls()).filter(call => call.method === "x-air-app/openView")
    expect(opened).toEqual([expect.objectContaining({ params: expect.objectContaining({ view: "review" }) })])

    // the files of the branch came as the call events of `get_file_diffs`, and the call answered after the last
    await expect(review.getByTestId("review-file-header")).toHaveAttribute("data-path", "auth.js")
  })

  test("Escape in a review tab closes its place: brd asks the IDE to close the view of its instance", async ({ ide }) => {
    const review = await openFeatureReview(ide)
    await expect(review.getByTestId("review-file-header")).toHaveAttribute("data-path", "auth.js")

    // the user works in the tab first: a frame of another origin takes the focus only from the user
    await review.getByTestId("review-file-header").click()
    await ide.page.keyboard.press("Escape")
    // the close of the window of the page is `x-air-app/closeView` of its instance, and the IDE closed the tab
    await expect.poll(async () => (await ide.views()).map(view => view.instance)).toEqual(["main:1"])
    expect((await ide.calls()).filter(call => call.method === "x-air-app/closeView")).toEqual([{ method: "x-air-app/closeView", params: { instance: "review:1" } }])
  })

  test("the actions in the title of the tool window: Branch Deck Command Palette opens the palette of the main view, Sync Virtual Branches syncs its repository", async ({
    ide,
  }) => {
    const main = ide.frame("main:1")
    await expect(main.getByTestId("sync-button")).toBeEnabled()
    await ide.runAction("command-palette", "open_command_palette")
    // brd sent the main view the event of the palette, as the menu of the desktop app does
    await expect(main.getByTestId("command-palette")).toBeVisible()
    await main.getByTestId("command-palette").press("Escape")
    await expectClosed(main.getByTestId("command-palette"))

    const answer = (await ide.runAction("sync", "sync")) as { structuredContent: { value: { outcome: string } } }
    expect(["synced", "unchanged"]).toContain(answer.structuredContent.value.outcome)
    await expect(main.locator('[data-testid="branch-row"][data-branch-name="feature"]')).toBeVisible()
  })

  test("Settings → Tools in the IDE shows the command-line tools but no install of brd or of IntelliJ IDEA, which the desktop app installs", async ({ ide }) => {
    const instance = await ide.openView("settings", { data: { page: "tools" } })
    const settings = ide.frame(instance)
    await expect(settings.getByTestId("cli-tool-teamcity")).toBeVisible()
    // brd mcp installs nothing: its status says so, and the page shows neither row
    await expect(settings.getByTestId("agent-skill")).toHaveCount(0)
    await expect(settings.getByTestId("ide-app")).toHaveCount(0)
  })

  test("the selector of the main view lists the Git roots of the project, and the root picked shows again at the next start of the project", async ({ page, repo }) => {
    const other = new TestRepositoryBuilder().useTemplate("simple")
    await other.init()
    try {
      const nameOf = (path: string) => path.split("/").at(-1)!
      const roots = [repo.path, other.path]
      let main = (await openFakeIde(page, { repoId: repo.id, repositoryPath: repo.path, roots })).frame("main:1")
      const selector = () => main.getByRole("combobox")
      await expect(selector()).toContainText(nameOf(repo.path))
      await selector().click()
      await expect(main.getByRole("option")).toHaveText([new RegExp(nameOf(repo.path)), new RegExp(nameOf(other.path))])
      await main.getByRole("option", { name: nameOf(other.path) }).click()
      await expect(selector()).toContainText(nameOf(other.path))

      // the next start of the project: brd remembered the root picked for these roots, whatever their order
      main = (await openFakeIde(page, { repoId: repo.id, repositoryPath: repo.path, roots: [...roots].reverse() })).frame("main:1")
      await expect(selector()).toContainText(nameOf(other.path))
    } finally {
      await other.cleanup()
    }
  })

  test("the page shows the theme of the IDE and follows its switch from Light to Dark without a reload", async ({ ide }) => {
    const main = ide.frame("main:1")
    const root = main.locator("html")
    const token = (name: string) => root.evaluate((element, it) => getComputedStyle(element).getPropertyValue(it).trim().toLowerCase(), name)
    const variable = (theme: HostContextTheme, name: string) => theme.styles.variables[name]!.toLowerCase()
    await expect(root).toHaveAttribute("data-theme", "light")
    // the canvas is the background of the place, the cards the raised surface of the IDE
    await expect.poll(() => token("--canvas")).toBe(variable(IDE_THEMES.light, "--color-background-primary"))
    await expect.poll(() => token("--card")).toBe(variable(IDE_THEMES.light, "--color-background-secondary"))
    // a mark of this document: a reload would lose it
    await root.evaluate(element => element.setAttribute("data-e2e-document", "first"))

    await ide.setTheme(IDE_THEMES.dark)
    await expect(root).toHaveAttribute("data-theme", "dark")
    await expect.poll(() => token("--canvas")).toBe(variable(IDE_THEMES.dark, "--color-background-primary"))
    await expect.poll(() => token("--card")).toBe(variable(IDE_THEMES.dark, "--color-background-secondary"))
    // in the dark theme too the canvas is darker than the cards on it
    const lightness = (hex: string) => [1, 3, 5].reduce((sum, at) => sum + Number.parseInt(hex.slice(at, at + 2), 16), 0)
    expect(lightness(await token("--canvas"))).toBeLessThan(lightness(await token("--card")))
    await expect(root).toHaveAttribute("data-e2e-document", "first")
  })
})
