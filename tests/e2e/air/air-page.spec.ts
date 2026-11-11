// @spec openspec/specs/repository/ide-embedding/spec.md
import { test as base } from "@playwright/test"
import { IDE_THEMES, openFakeIde } from "../air-host/fake-host"
import type { FakeIde } from "../air-host/fake-host"
import { expect } from "../fixtures/base-test"
import { TestRepositoryBuilder } from "../helpers/test-repository"

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

test.describe("Branch Deck in the IDE", () => {
  test("the tool window shows the branches of the repository after a sync of the page, each patch of the mirror a change of its rows", async ({ ide }) => {
    const main = ide.frame("main:1")
    const sync = main.getByTestId("sync-button")
    await expect(sync).toBeEnabled()
    const before = Number(await sync.getAttribute("data-sync-count"))
    await sync.click()
    // the command ran through bd_invoke, and the patches of its run reached the view by the mirror
    await expect(sync).not.toHaveAttribute("data-sync-count", String(before))
    await expect(main.locator('[data-testid="branch-row"][data-branch-name="feature"]')).toBeVisible()
  })

  test("Review Changes opens the review as a view of the IDE, whose diffs stream as the call events of its call", async ({ ide }) => {
    const main = ide.frame("main:1")
    await main.getByTestId("sync-button").click()
    const row = main.locator('[data-testid="branch-row"][data-branch-name="feature"]')
    await expect(row).toBeVisible()
    await row.click({ button: "right" })
    await main.getByRole("menuitem", { name: "Review Changes" }).click()

    // brd asked the IDE for the view of the review (`x-air-app/openView`), and the IDE opened its place
    await expect.poll(async () => (await ide.views()).map(view => view.view)).toEqual(["main", "review"])
    const opened = (await ide.calls()).filter(call => call.method === "x-air-app/openView")
    expect(opened).toEqual([expect.objectContaining({ params: expect.objectContaining({ view: "review" }) })])

    // the files of the branch came as the call events of `get_file_diffs`, and the call answered after the last
    const review = ide.frame("review:1")
    await expect(review.getByTestId("review-file-header")).toHaveAttribute("data-path", "auth.js")
  })

  test("the page shows the theme of the IDE and follows its switch from Light to Dark without a reload", async ({ ide }) => {
    const main = ide.frame("main:1")
    const root = main.locator("html")
    const token = (name: string) => root.evaluate((element, it) => getComputedStyle(element).getPropertyValue(it).trim().toLowerCase(), name)
    await expect(root).toHaveAttribute("data-theme", "light")
    await expect.poll(() => token("--background")).toBe(IDE_THEMES.light.styles.variables["--color-background-primary"]!.toLowerCase())
    // a mark of this document: a reload would lose it
    await root.evaluate(element => element.setAttribute("data-e2e-document", "first"))

    await ide.setTheme(IDE_THEMES.dark)
    await expect(root).toHaveAttribute("data-theme", "dark")
    await expect.poll(() => token("--background")).toBe(IDE_THEMES.dark.styles.variables["--color-background-primary"]!.toLowerCase())
    await expect(root).toHaveAttribute("data-e2e-document", "first")
  })
})
