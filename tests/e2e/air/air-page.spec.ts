// @spec openspec/specs/repository/ide-embedding/spec.md
import { test as base } from "@playwright/test"
import type { FrameLocator, Page } from "@playwright/test"
import { closeFakeIde, IDE_THEMES, openFakeIde } from "../air-host/fake-host"
import type { FakeIde } from "../air-host/fake-host"
import type { HostContextTheme } from "../air-host/host-page"
import { expect } from "../fixtures/base-test"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { moveClockUntil } from "../helpers/sync-helpers"
import { TestRepositoryBuilder } from "../helpers/test-repository"
import { expectClosed } from "../helpers/wait-helpers"

/*
 * The Air build in a fake IDE (`tests/e2e/air-host/`): the frame of each view loads `dist-air/`, the host page relays the
 * frame link of MCP Apps with `x-air-app` to `brd mcp` of the test server (`mcp-host`) over its WebSocket. No mock of
 * Tauri: the page runs on its shim (`src/platform/air/`). The fake IDE keeps the rules of the page connection of Air (one
 * view for each frame, the calls that wait), and every test ends with none of its calls refused
 */
/**
 * A repository of the test from `template`, which goes after the IDE closed, so no session that closes ends a mirror a
 * view still shows; the test fails when a call of its views broke a rule of the page connection of Air
 */
async function repositoryOf(page: Page, template: string, use: (repo: TestRepositoryBuilder) => Promise<void>): Promise<void> {
  const repo = new TestRepositoryBuilder().useTemplate(template)
  await repo.init()
  await use(repo)
  const refusals = await closeFakeIde(page)
  await repo.cleanup()
  expect(refusals).toEqual([])
}

const test = base.extend<{ ide: FakeIde; repo: TestRepositoryBuilder; other: TestRepositoryBuilder; unprefixed: TestRepositoryBuilder }>({
  // `feature` with auth.js, and the unassigned "Fix critical bug" with bugfix.txt
  repo: ({ page }, use) => repositoryOf(page, "single_unassigned", use),
  // a second Git root of the project: `test-branch` with one commit
  other: ({ page }, use) => repositoryOf(page, "simple", use),
  // a repository whose git config names no branch prefix
  unprefixed: ({ page }, use) => repositoryOf(page, "simple_no_prefix", use),
  ide: async ({ page, repo }, use) => {
    await use(await openFakeIde(page, { repoId: repo.id, repositoryPath: repo.path }))
  },
})

/** The name of a repository in the selector of the main view: the last segment of its path */
function nameOf(path: string): string {
  return path.split("/").at(-1)!
}

/** Syncs a view by its button and waits for the end of the sync: its count moved, and the button is not busy */
async function syncView(view: FrameLocator): Promise<void> {
  const sync = view.getByTestId("sync-button")
  await expect(sync).toBeEnabled()
  const before = Number(await sync.getAttribute("data-sync-count"))
  await sync.click()
  await expect(sync).not.toHaveAttribute("data-sync-count", String(before))
  await expect(sync).not.toHaveAttribute("aria-busy", "true")
}

/** Syncs the main view by its button and waits for the end of the sync: the row of the branch `feature`, settled */
async function syncedFeatureRow(ide: FakeIde) {
  const main = ide.frame("main:1")
  await syncView(main)
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
    // both views of the repository subscribed to its one mirror, one channel of the page link
    const subscribed = (await ide.mirrorLog()).filter(entry => entry.kind === "subscribed")
    expect(subscribed.map(entry => entry.instance)).toEqual(["main:1", "review:1"])
    expect(new Set(subscribed.map(entry => entry.channel)).size).toBe(1)
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

  test("the main view in the IDE offers no Open Repository, as the IDE has no folder dialog: the roots of the project are the repositories of its selector", async ({ ide }) => {
    const main = ide.frame("main:1")
    // the selector of the roots shows, without the folder button beside it
    await expect(main.getByRole("combobox")).toBeVisible()
    await expect(main.getByTestId("browse-repository-button")).toHaveCount(0)
    // nor does the command palette offer it, beside its other commands
    await ide.runAction("command-palette", "open_command_palette")
    const palette = main.getByTestId("command-palette")
    await expect(palette.getByRole("option", { name: "Activity Window" })).toBeVisible()
    await expect(palette.getByRole("option", { name: "Open Repository…" })).toHaveCount(0)
  })

  test("the selector of the main view lists the Git roots of the project, and the root picked shows again at the next start of the project", async ({ page, repo, other }) => {
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
  })

  test("a pick of another root before the first subscription of the main view answered subscribes to it once that subscription answered and ended: the frame holds one view", async ({
    page,
    repo,
    other,
  }) => {
    const ide = await openFakeIde(page, { repoId: repo.id, repositoryPath: repo.path, roots: [repo.path, other.path], holdSubscriptions: true })
    const main = ide.frame("main:1")
    // the main view subscribes to the first root, which waits before brd
    await expect.poll(() => ide.heldSubscriptions()).toBe(1)
    await main.getByRole("combobox").click()
    await main.getByRole("option", { name: nameOf(other.path) }).click()
    await expect(main.getByRole("combobox")).toContainText(nameOf(other.path))
    expect(await ide.heldSubscriptions()).toBe(1)

    await ide.releaseSubscriptions()
    // the view of the first root answered and ended before the picked one subscribed
    await expect
      .poll(async () => (await ide.mirrorLog()).map(entry => [entry.kind, entry.mirror]))
      .toEqual([
        ["subscribed", repo.path],
        ["unsubscribed", repo.path],
        ["subscribed", other.path],
      ])
    // the view of the picked root is live: its sync shows its branch
    await syncView(main)
    await expect(main.locator('[data-testid="branch-row"][data-branch-name="test-branch"]')).toBeVisible()
  })

  test("a prefix set in a terminal that a sync finds moves the main view to the session of the new prefix: the frame keeps its one view, which brd moves before it answers the subscription again", async ({
    ide,
    repo,
  }) => {
    await syncedFeatureRow(ide)
    const main = ide.frame("main:1")
    const sync = main.getByTestId("sync-button")
    const epoch = async () => (await sync.getAttribute("data-view-version"))?.split(":")[0]
    const synced = await epoch()
    expect(synced).toBeDefined()

    await repo.setBranchPrefixInTerminal("team")
    // the run reads the prefix of the config: `prefix_changed`, and the main view subscribes again holding its view
    await sync.click()
    await expect.poll(epoch).not.toBe(synced)
    // the session of the new prefix syncs as it opens, for a repository synced before
    await expect(main.locator('[data-testid="branch-row"][data-branch-name="feature"]')).toBeVisible()
    await expect(sync).not.toHaveAttribute("aria-busy", "true")
    await expect(sync).toBeEnabled()
    expect(await repo.revision("team/virtual/feature")).toMatch(/^[0-9a-f]{40}$/)
    // one view all along: the main view subscribed once, and its mirror moved without an end
    expect((await ide.mirrorLog()).map(entry => [entry.instance, entry.kind])).toEqual([["main:1", "subscribed"]])
  })

  test("a repository without a branch prefix shows the welcome card in the main view, as brd refused its subscription with no_branch_prefix, and Save shows the repository", async ({
    page,
    unprefixed,
  }) => {
    const main = (await openFakeIde(page, { repoId: unprefixed.id, repositoryPath: unprefixed.path })).frame("main:1")
    // the ApiError of brd reached the page: the prefix step with the suggestion of its details, not an error
    const welcomeCard = main.getByTestId("welcome-card")
    await expect(welcomeCard.getByRole("heading", { name: "Choose your branch prefix" })).toBeVisible()
    await expect(welcomeCard.getByRole("textbox", { name: "Branch prefix to save" })).toHaveValue("test")

    await welcomeCard.getByRole("button", { name: "Save" }).click()
    await expectClosed(welcomeCard)
    await expect(main.getByTestId("sync-button")).toBeEnabled()
    await expect(main.getByTestId("ready-to-sync-card")).toBeVisible()
  })

  test("a mirror that Air ends while brd keeps the session of its repository is subscribed to again, so the main view shows what a sync after the end changes", async ({
    ide,
    repo,
  }) => {
    await syncedFeatureRow(ide)
    await ide.closeMirror(repo.path)
    // the snapshot of a mirror that ended is none, as of a session that went away: the main view subscribed again
    await expect
      .poll(async () => (await ide.mirrorLog()).map(entry => [entry.instance, entry.kind]))
      .toEqual([
        ["main:1", "subscribed"],
        ["main:1", "ended"],
        ["main:1", "subscribed"],
      ])

    // its new subscription is live: a commit of a new group is a row after a sync
    await repo.commit("(pool) Add the pool config", { "pool.cfg": "size=10\n" }, 1704182400)
    const main = ide.frame("main:1")
    await syncView(main)
    await expect(main.locator('[data-testid="branch-row"][data-branch-name="pool"]')).toBeVisible()
  })

  test("a Safe Push that fails while the IDE shows another tab is a notification of the IDE, whose action shows the branch in the tool window", async ({ page, repo }) => {
    const running = robot("feature", "robot-feature")
    await repo.setPatronus(availableRobots(running))
    const ide = await openFakeIde(page, { repoId: repo.id, repositoryPath: repo.path })
    const row = await syncedFeatureRow(ide)
    await expect(row.getByTestId("patronus-chip")).toBeVisible()

    // another tab shows: the tool window is hidden when the robot fails
    await ide.hideView("main:1")
    await repo.setPatronus(availableRobots({ ...running, status: "failure", finishTime: 1704189000 }))
    const notifications = async () => (await ide.calls()).filter(call => call.method === "x-air-app/notify")
    await moveClockUntil(repo, 5 * 60_000, async () => (await notifications()).length > 0)
    expect(await notifications()).toEqual([
      {
        method: "x-air-app/notify",
        params: {
          title: "Safe Push of feature failed",
          body: "feature → master",
          kind: "error",
          actions: [{ text: "Show in Branch Deck", run: { tool: "reveal_branch", arguments: { repositoryPath: repo.path, branchName: "feature" } } }],
        },
      },
    ])

    // Show in Branch Deck: brd asks the IDE to show the tool window, and the main view reveals the branch in it
    await ide.runNotificationAction(0)
    await expect(row).toBeFocused()
    expect((await ide.calls()).filter(call => call.method === "x-air-app/openView")).toEqual([{ method: "x-air-app/openView", params: { view: "main" } }])
    expect((await ide.views()).map(view => view.instance)).toEqual(["main:1"])
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
