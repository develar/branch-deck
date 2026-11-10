import type { Page, Locator } from "@playwright/test"
import type { ApiError, StepFact } from "../../../src/lib/bindings"
import { expect } from "../fixtures/base-test"

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

/** A popup as it closes: Base UI marks it `data-closed` at once (a toast `data-ending-style`), while its exit runs */
const CLOSING = "[data-closed], [data-ending-style]"

/**
 * `popup` (a dialog, a menu, a popover, a card, a tooltip, a toast, or what lies in one) closed, as the user sees it.
 * Base UI marks a closing popup `data-closed` at once, keeps it mounted while its exit animation runs, then unmounts it
 * and gives the focus back. The exit ends only as the window paints, and a loaded machine paints no frame of a window
 * for seconds: a wait for the popup to leave the page timed out after it had closed. So this waits for the state the
 * close set (each element of `popup` closing or not shown; none: closed), then ends the exit as the frames would
 * (`finish()` of its animations, which needs no frame; again on each try, as a closing toast starts the slide of its
 * neighbours) until nothing of it shows: the popup is unmounted and the focus back by then. Not in a test of a popup
 * reopened during its exit, which needs the exit running
 */
export async function expectClosed(popup: Locator): Promise<void> {
  await expect
    .poll(() => popup.evaluateAll((nodes, closing) => nodes.every(node => node.closest(closing) !== null || !node.checkVisibility()), CLOSING), {
      message: `${String(popup)} closes`,
    })
    .toBe(true)
  await expect
    .poll(
      () =>
        popup.evaluateAll((nodes, closing) => {
          for (const node of nodes) {
            for (const animation of node.closest(closing)?.getAnimations({ subtree: true }) ?? []) {
              if (animation.effect?.getComputedTiming().iterations !== Infinity) {
                animation.finish()
              }
            }
          }
          return nodes.filter(node => node.checkVisibility()).length
        }, CLOSING),
      { message: `${String(popup)} ends its exit` },
    )
    .toBe(0)
}

/**
 * Resolves once a rendering update of the page delivered its `ResizeObserver` reports of what changed before the call: a
 * frame runs its animation frame callbacks before its layout and its reports, so the second callback is after them. A
 * read of a width laid out (`boundingBox`) forces the layout at once and is no such update
 */
export async function resizeReportsDelivered(page: Page): Promise<void> {
  await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))))
}

/**
 * The facts a step waits for (`StepFact` of Rust, `PendingFact` of `src/presenters/next-step.ts`), in the order the step
 * waits for them: the keys of a record of every fact, so a new one joins the wait
 */
const STEP_FACTS = Object.keys({
  patronus: true,
  pullRequests: true,
  landingNeeds: true,
  robotProblems: true,
  readiness: true,
  permission: true,
  targetRules: true,
  mergeState: true,
} satisfies Record<StepFact, true>)

/**
 * Waits until no step of the next-step `slots` waits for a fact (`data-waits-for` of "Checking…"), each fact with an
 * assertion's time of its own: under load every answer takes seconds and they come one after another (the landing needs
 * after the sync, the approvals after the robots), so one assertion over all of them failed at a load of 150-200. A
 * second round takes a fact asked once another answered. A step pending on a failed read (no fact) is the caller's
 */
export async function waitForStepFacts(slots: Locator): Promise<void> {
  const waiting = (fact?: string) => slots.and(slots.page().locator(fact === undefined ? "[data-waits-for]" : `[data-waits-for="${fact}"]`))
  for (let round = 0; round < 2; round++) {
    for (const fact of STEP_FACTS) {
      await expect(waiting(fact)).toHaveCount(0)
    }
  }
  await expect(waiting()).toHaveCount(0)
}

/**
 * Holds every call of a Tauri command (e.g. `sync`) in the mock until the returned function releases them, to
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
 * second surface) counts what reached the backend. With `scope: "context"` the hold is on the context, so it holds the
 * calls of the windows opened after it too: a command the page answers from a cache (`get_blob_texts`, kept by blob id
 * in `src/stores/blob-texts.ts`) is asked once, and a hold installed after the window opened may never see it
 */
export async function holdCommandCalls(
  page: Page,
  command: string,
  { scope = "page" }: { scope?: "page" | "context" } = {},
): Promise<{ calls: Record<string, unknown>[]; release: () => void }> {
  const calls: Record<string, unknown>[] = []
  let release!: () => void
  const held = new Promise<void>(resolve => {
    release = resolve
  })
  const router = scope === "context" ? page.context() : page
  await router.route(`**/invoke/${command}`, async route => {
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

/** A rewrite of the engine the page asked (`execute` of the intent `rewrite`): its repository and the rewrite */
export interface RewriteCall {
  readonly repositoryPath: string
  /** `{ rewrite: "unstack", branchName, witness }`, the `kind` of the intent left out */
  readonly rewrite: Record<string, unknown>
}

/** The rewrite of the `execute` `params` of the page, of `kind` (every rewrite without it); null for another intent */
function rewriteCall(params: Record<string, unknown>, kind: string | undefined): RewriteCall | null {
  const { kind: family, ...rewrite } = (params.intent ?? {}) as Record<string, unknown>
  return family === "rewrite" && (kind === undefined || rewrite.rewrite === kind) ? { repositoryPath: params.repositoryPath as string, rewrite } : null
}

/**
 * Records each `execute` of the page that runs a rewrite of the engine (`kind`: `"move"`, `"park"`, `"undo"`; every
 * rewrite without it), which the test server runs: a test of a rewrite that must run once counts what reached the
 * backend
 */
export async function recordRewrites(page: Page, kind?: string): Promise<RewriteCall[]> {
  const calls: RewriteCall[] = []
  await page.route("**/invoke/execute", async route => {
    const call = rewriteCall(route.request().postDataJSON() as Record<string, unknown>, kind)
    if (call !== null) {
      calls.push(call)
    }
    await route.fallback()
  })
  return calls
}

/**
 * Holds each `execute` of a rewrite of the engine (`kind`, every rewrite without it) in the page mock until `release`,
 * recording it: the page shows the rewrite running while the session has not got it yet; the other intents go on. A
 * rewrite that must wait in the queue of the session holds behind the repository lock instead
 * (`TestRepositoryBuilder.holdRepositoryLock`)
 */
export async function holdRewrites(page: Page, kind?: string): Promise<{ calls: RewriteCall[]; release: () => void }> {
  const calls: RewriteCall[] = []
  let release!: () => void
  const held = new Promise<void>(resolve => {
    release = resolve
  })
  await page.route("**/invoke/execute", async route => {
    const call = rewriteCall(route.request().postDataJSON() as Record<string, unknown>, kind)
    if (call !== null) {
      calls.push(call)
      await held
    }
    await route.fallback()
  })
  return { calls, release }
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

/** The commands of `page` whose answer has not come yet (`__ipcStats` of the page mock): 0 once a streamed one ended */
export function pendingCommands(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as { __ipcStats: { pending: number } }).__ipcStats.pending)
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
 * Wait for tooltip to appear with specific text
 */
export async function waitForTooltip(page: Page, text: string, timeout = 5000): Promise<Locator> {
  const tooltip = page.locator(`[role="tooltip"][data-open]:has-text("${text}")`)
  await expect(tooltip).toBeVisible({ timeout })
  return tooltip
}

/**
 * Wait for tooltip text to change
 */
export async function waitForTooltipChange(page: Page, fromText: string, toText: string, timeout = 5000): Promise<void> {
  // Wait for original tooltip to disappear
  const originalTooltip = page.locator(`[role="tooltip"][data-open]:has-text("${fromText}")`)
  await expect(originalTooltip).not.toBeVisible({ timeout })

  // Wait for new tooltip to appear
  const newTooltip = page.locator(`[role="tooltip"][data-open]:has-text("${toText}")`)
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
 * Holds each `execute` of the intent of `kind` (`"updateProject"`, `"fetchBaseline"`) in the page mock until `release`,
 * recording its intent; `send` reports events of its operation on its channel meanwhile (`OperationEvent`: a `step`, a
 * `progress` of its fetch), as the session tells them, then the test server runs it. The other intents go on
 */
export async function holdIntent(page: Page, kind: string): Promise<{ calls: Record<string, unknown>[]; send: (...events: unknown[]) => Promise<void>; release: () => void }> {
  const calls: Record<string, unknown>[] = []
  let release!: () => void
  const held = new Promise<void>(resolve => (release = resolve))
  let channelOf!: (channel: string) => void
  const channel = new Promise<string>(resolve => (channelOf = resolve))
  await page.route("**/invoke/execute", async route => {
    const intent = ((route.request().postDataJSON() as Record<string, unknown>).intent ?? {}) as Record<string, unknown>
    if (intent.kind === kind) {
      calls.push(intent)
      channelOf(route.request().headers()["x-progress-channel"]!)
      await held
    }
    await route.fallback()
  })
  return { calls, send: async (...events) => sendProgress(page, await channel, events), release }
}

/**
 * The open tooltip of `page`, by its role (`options` of `getByRole`): a tooltip that closes keeps its role while its exit
 * runs, which ends only as the window paints, so under load a plain `getByRole("tooltip")` finds the tooltip of the hover
 * before beside the new one (a strict mode violation), and a check that no tooltip shows waits for that exit
 */
export function openTooltip(page: Page, options?: Parameters<Page["getByRole"]>[1]): Locator {
  return page.getByRole("tooltip", options).and(page.locator("[data-open]"))
}
