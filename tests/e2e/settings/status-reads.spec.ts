// @spec spec/repository/settings.spec.md
import type { Locator, Page } from "@playwright/test"
import type { SettingsTarget } from "../../../src/lib/settings-window"
import { test, expect } from "../fixtures/test-fixtures"
import { openSettingsWindow } from "../helpers/settings-helpers"
import type { HeldAnswer, TestRepositoryBuilder } from "../helpers/test-repository"

interface StatusRead {
  page: NonNullable<SettingsTarget["page"]>
  answer: HeldAnswer
  /** The line while the read runs */
  reading: string
  /** The callout of a failed read */
  failure: { testId: string; title: string }
  /** What the page shows once the status answered */
  status: (section: Locator) => Locator
}

/**
 * A page of Settings before the answer of its status and after a failed one: it says it reads while the fake holds the
 * read, the failure says why with Retry, and Retry reads again until the status answers
 */
async function expectStatusRead(page: Page, repo: TestRepositoryBuilder, read: StatusRead): Promise<void> {
  await repo.holdRemoteAnswers({ hold: [read.answer] })
  const settings = await openSettingsWindow(page, { page: read.page })
  const section = settings.getByTestId(`settings-page-${read.page}`)
  await expect.poll(() => repo.getArrivedAnswers()).toContain(read.answer)
  await expect(section.getByText(read.reading)).toBeVisible()
  // the line above anchors it: the section is there, the status is not
  await expect(read.status(section)).toHaveCount(0)

  // the held read fails
  await repo.holdRemoteAnswers({ fail: [read.answer], release: [read.answer] })
  const failure = section.getByTestId(read.failure.testId)
  await expect(failure).toContainText(read.failure.title)
  await expect(failure).toContainText("the fixture fails this read")
  await expect(section.getByText(read.reading)).toHaveCount(0)
  await expect(read.status(section)).toHaveCount(0)

  // Retry reads again, and this time the answer comes
  await repo.holdRemoteAnswers({ release: [read.answer] })
  await failure.getByRole("button", { name: "Retry" }).click()
  await expect(read.status(section)).toBeVisible()
  await expect(failure).toHaveCount(0)
}

test.describe("Settings: the read of a status", () => {
  test("should say it reads the GitHub sign-in until the answer, and a failed read with Retry", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await expectStatusRead(page, repo, {
      page: "github",
      answer: "gitHubAuth",
      reading: "Reading the GitHub sign-in…",
      failure: { testId: "github-auth-error", title: "Cannot read the GitHub sign-in" },
      status: section => section.getByTestId("github-auth"),
    })
  })

  test("should say it reads the YouTrack sign-in until the answer, and a failed read with Retry", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await expectStatusRead(page, repo, {
      page: "youtrack",
      answer: "youTrackAuth",
      reading: "Reading the YouTrack sign-in…",
      failure: { testId: "youtrack-auth-error", title: "Cannot read the YouTrack sign-in" },
      status: section => section.getByTestId("youtrack-auth"),
    })
  })

  test("should say it reads the tools until the answer, and a failed read with Retry", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await expectStatusRead(page, repo, {
      page: "tools",
      answer: "cliTools",
      reading: "Reading the tools…",
      failure: { testId: "cli-tools-error", title: "Cannot read the tools" },
      status: section => section.getByTestId("cli-tool-teamcity"),
    })
  })
})
