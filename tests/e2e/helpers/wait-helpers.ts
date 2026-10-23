import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"
import type { ApiError } from "../../../src/lib/bindings"

/**
 * Wait for processing state to complete (animate-pulse class removed)
 */
export async function waitForProcessingComplete(page: Page, element: Locator): Promise<void> {
  // Check if element is currently processing
  const elementClasses = await element.getAttribute("class")
  const isProcessing = elementClasses?.includes("animate-pulse")

  if (isProcessing) {
    // Wait for processing to complete - the animate-pulse class should be removed
    await expect(element).not.toHaveClass(/animate-pulse/)
  }
}

/**
 * Resolves once the animations of `elements` and their subtrees ended (an infinite one, a spinner, is left out): a popup
 * zooms or slides in as it opens, and a pointer put on it meanwhile ends on what the animation moves under it
 */
export async function animationsEnded(elements: Locator): Promise<void> {
  await elements.evaluateAll(nodes =>
    Promise.all(
      nodes
        .flatMap(node => node.getAnimations({ subtree: true }))
        .filter(animation => animation.effect?.getComputedTiming().iterations !== Infinity)
        .map(animation => animation.finished.catch(() => null)),
    ),
  )
}

/**
 * Holds every call of a Tauri command (e.g. `sync_branches`) in the mock until the returned function releases them, to
 * check the page while the command runs; then it goes on to the routes of the command registered before (a rewrite of
 * its answer), else to the test server
 */
export async function holdCommand(page: Page, command: string): Promise<() => void> {
  let release!: () => void
  const held = new Promise<void>(resolve => {
    release = resolve
  })
  await page.route(`**/invoke/${command}`, async route => {
    await held
    await route.fallback()
  })
  return release
}

/**
 * `holdCommand` that records the parameters of each call: a test of an action that must run once (a double click, a
 * second surface) counts what reached the backend
 */
export async function holdCommandCalls(page: Page, command: string): Promise<{ calls: Record<string, unknown>[]; release: () => void }> {
  const calls: Record<string, unknown>[] = []
  let release!: () => void
  const held = new Promise<void>(resolve => {
    release = resolve
  })
  await page.route(`**/invoke/${command}`, async route => {
    calls.push(route.request().postDataJSON() as Record<string, unknown>)
    await held
    await route.fallback()
  })
  return { calls, release }
}

/**
 * Lets the next call of a command run on the test server and holds its answer until `release`: `ran` resolves once the
 * server answered, so a test can start another call meanwhile and see the older answer arrive last
 */
export async function holdNextAnswer(page: Page, command: string): Promise<{ ran: Promise<void>; release: () => void }> {
  let release!: () => void
  const held = new Promise<void>(resolve => {
    release = resolve
  })
  let answered!: () => void
  const ran = new Promise<void>(resolve => {
    answered = resolve
  })
  await page.route(
    `**/invoke/${command}`,
    async route => {
      const response = await route.fetch()
      answered()
      await held
      await route.fulfill({ response })
    },
    { times: 1 },
  )
  return { ran, release }
}

/**
 * Answers a command in the page instead of the test server, and records the parameters of each call; `answer` may
 * resolve later (read what the test server holds at that moment)
 */
export async function routeCommand(page: Page, command: string, answer: (params: Record<string, unknown>) => unknown): Promise<Record<string, unknown>[]> {
  const calls: Record<string, unknown>[] = []
  await page.route(`**/invoke/${command}`, async route => {
    const params = route.request().postDataJSON() as Record<string, unknown>
    calls.push(params)
    await route.fulfill({ json: await answer(params) })
  })
  return calls
}

/**
 * Records the parameters of each call of `command`, which the test server answers: what the page sent that the fake
 * of the remote service never sees (the overrides of a start)
 */
export async function recordCommand(page: Page, command: string): Promise<Record<string, unknown>[]> {
  const calls: Record<string, unknown>[] = []
  await page.route(`**/invoke/${command}`, async route => {
    calls.push(route.request().postDataJSON() as Record<string, unknown>)
    await route.fallback()
  })
  return calls
}

/**
 * Fails `command` with an `ApiError`, as the mock server answers a failed command and Tauri rejects it: the code the
 * page branches on (`internal` for a failure without one), the message for the user, the data of the code (`details`,
 * null by default) and its retry class (`no` by default; `locked` is `same`, a `*_moved` code `after_reread`). `times`
 * fails only that many calls; the test server answers the next (a retry that passes)
 */
export async function routeCommandError(
  page: Page,
  command: string,
  error: Pick<ApiError, "code" | "message"> & Partial<Pick<ApiError, "details" | "retry">>,
  options?: { readonly times?: number },
): Promise<void> {
  const answer: ApiError = { details: null, retry: "no", operation_id: null, ...error }
  await page.route(`**/invoke/${command}`, route => route.fulfill({ json: { status: "error", error: answer } }), options)
}

/**
 * Wait for a specific API response
 */
export async function waitForApiResponse(page: Page, urlPattern: string | RegExp, expectedStatus = 200): Promise<void> {
  await page.waitForResponse(response => {
    const matches = typeof urlPattern === "string" ? response.url().includes(urlPattern) : urlPattern.test(response.url())
    return matches && response.status() === expectedStatus
  })
}

/**
 * Wait for element with specific text
 */
export async function waitForElementWithText(page: Page, text: string, options?: { timeout?: number }): Promise<void> {
  await page.waitForSelector(`text=${text}`, options)
}

/**
 * Wait for branch details to expand
 */
export async function waitForBranchDetailsExpanded(page: Page, branchName: string): Promise<Locator> {
  // Wait for the branch details row to be visible
  const detailsRow = page.locator(`[data-branch-name="${branchName}"] ~ tr`)
  await detailsRow.waitFor({ state: "visible" })
  return detailsRow
}

/**
 * Wait with timeout and return a default value if timeout
 */
export async function waitWithTimeout<T>(promise: Promise<T>, timeoutMs: number, defaultValue: T): Promise<T> {
  return Promise.race([promise, new Promise<T>(resolve => setTimeout(() => resolve(defaultValue), timeoutMs))])
}

/**
 * Wait for tooltip to appear with specific text
 */
export async function waitForTooltip(page: Page, text: string, timeout = 5000): Promise<Locator> {
  const tooltip = page.locator(`[role="tooltip"]:has-text("${text}")`)
  await expect(tooltip).toBeVisible({ timeout })
  return tooltip
}

/**
 * Wait for tooltip text to change
 */
export async function waitForTooltipChange(page: Page, fromText: string, toText: string, timeout = 5000): Promise<void> {
  // Wait for original tooltip to disappear
  const originalTooltip = page.locator(`[role="tooltip"]:has-text("${fromText}")`)
  await expect(originalTooltip).not.toBeVisible({ timeout })

  // Wait for new tooltip to appear
  const newTooltip = page.locator(`[role="tooltip"]:has-text("${toText}")`)
  await expect(newTooltip).toBeVisible({ timeout })
}

/**
 * Wait for conflict viewer to load in branch details
 */
export async function waitForConflictViewer(page: Page, branchDetailsRow: Locator, timeout = 10000): Promise<Locator> {
  // Wait for the conflict title ("“<subject>” does not apply cleanly onto <base>") to appear
  const conflictTitle = branchDetailsRow.getByTestId("conflict-title").first()
  await expect(conflictTitle).toContainText("does not apply cleanly onto", { timeout })

  // the conflicting files follow the title
  const conflictingFiles = branchDetailsRow.getByTestId("conflicting-files").first()
  await expect(conflictingFiles).toBeVisible({ timeout })
  // the fix of a sync conflict is looked for when the conflict is shown: wait until the search ends, so the view stands
  const fix = branchDetailsRow.getByTestId("conflict-fix")
  if ((await fix.count()) > 0) {
    await expect(fix.first()).not.toHaveAttribute("data-state", "searching", { timeout })
  }

  return conflictingFiles
}

/**
 * Wait for popover to appear
 */
export async function waitForPopover(page: Page, content: string | RegExp, timeout = 5000): Promise<Locator> {
  const popover = typeof content === "string" ? page.locator("div").filter({ hasText: content }).first() : page.locator("div", { hasText: content }).first()

  await expect(popover).toBeVisible({ timeout })
  return popover
}

/**
 * Waits until the settings have `key` as `matches` wants it, as a reload (a new window of the app) reads it: the
 * changes the page heard from the writer (`settings_changed`, remembered by the Tauri mock)
 */
export async function waitForStorePersistence(page: Page, key: string, matches: (value: unknown) => boolean): Promise<void> {
  await expect
    .poll(async () => {
      const written = await page.evaluate(name => (JSON.parse(sessionStorage.getItem("bd-e2e-store-writes") ?? "{}") as Record<string, unknown>)[name], key)
      return matches(written)
    })
    .toBe(true)
}

/**
 * Wait for a collapsible card to be fully expanded (animation complete)
 *
 * This helper waits for both the card container and its inner collapsible content
 * to have the 'data-state="open"' attribute, ensuring the card is fully expanded
 * before proceeding with further operations.
 *
 * @param card The card locator to wait for expansion
 *
 * @example
 * ```typescript
 * const card = page.getByTestId("archived-branches-card")
 * await card.getByText("Archived Branches", { exact: true }).click()
 * await waitForCollapsibleCardExpanded(card)
 * ```
 */
export async function waitForCollapsibleCardExpanded(card: Locator): Promise<void> {
  // Wait for the card container to be fully expanded
  await expect(card).toHaveAttribute("data-state", "open")

  // Also wait for the collapsible content to be fully expanded
  const collapsibleContent = card.locator(".overflow-x-auto[data-state]")
  await expect(collapsibleContent).toHaveAttribute("data-state", "open")
}

/**
 * Sends `messages` to the progress channel of a held command (its `X-Progress-Channel` header), as the backend reports
 * a fetch or a push
 */
export async function sendProgress(page: Page, channel: string, messages: readonly unknown[]): Promise<void> {
  await page.evaluate(
    ({ channel, messages }) => {
      // registered by the mock of the page (`registerProgressChannel` in tauri-mocks.ts)
      const channels = (window as unknown as { __progressChannels: Record<string, { onmessage: (message: unknown) => void }> }).__progressChannels
      for (const message of messages) {
        channels[channel]!.onmessage(message)
      }
    },
    { channel, messages },
  )
}

/**
 * Holds `command` in the page until `release`: `send` reports progress on its channel meanwhile, then the test server
 * runs it. Every call is held the same way.
 */
export async function holdWithProgress(page: Page, command: string): Promise<{ send: (...messages: unknown[]) => Promise<void>; release: () => void }> {
  let release!: () => void
  const held = new Promise<void>(resolve => (release = resolve))
  let channelOf!: (channel: string) => void
  const channel = new Promise<string>(resolve => (channelOf = resolve))
  await page.route(`**/invoke/${command}`, async route => {
    channelOf(route.request().headers()["x-progress-channel"]!)
    await held
    await route.continue()
  })
  return { send: async (...messages) => sendProgress(page, await channel, messages), release }
}
