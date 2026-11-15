import type { Locator, Page } from "@playwright/test"
import type { SettingsTarget } from "../../../src/lib/settings-window"
import { expect } from "../fixtures/base-test"
import { getOpenedSubWindows, openLinkedSubWindow, pressAndExpectOpened } from "./sub-window-helpers"
import type { OpenedSubWindow } from "./sub-window-helpers"
import { emitTauriEvent, nextAnswer, runAndWaitForAnswer, waitForTauriListener } from "./sync-helpers"

/**
 * Runs `press` (a click, a key, an event of the native menu), which makes `page` open Settings, and returns the Settings
 * window it opened as the `count`-th sub-window (so exactly one more than before), at `target`: the page and the field
 * the entry point asked for. The open is read once its command answered (`open_sub_window`, which the backend records
 * as it handles it and which takes seconds under load), never by a poll of the recorded opens, which gave up after 5 s
 */
export async function pressAndExpectSettingsOpened(page: Page, press: () => Promise<unknown>, target: SettingsTarget, count = 1): Promise<OpenedSubWindow<SettingsTarget>> {
  const opened = await pressAndExpectOpened<SettingsTarget>(page, press, "settings", count)
  expect(opened).toMatchObject({ url: "/settings", title: "Settings" })
  expect(opened.data).toEqual(target)
  return opened
}

/**
 * Opens Settings at `target` as the native menu does (`open_settings` to the main window), then shows the window as a
 * linked page of its own (`openLinkedSubWindow`): its writes of the settings reach the main window. `beforeLoad` routes
 * what the window asks the backend itself.
 */
export async function openSettingsWindow(page: Page, target: SettingsTarget = {}, options: { beforeLoad?: (window: Page) => Promise<unknown> } = {}): Promise<Page> {
  const count = (await getOpenedSubWindows(page)).length
  await waitForTauriListener(page, "open_settings")
  const opened = await pressAndExpectSettingsOpened(page, () => emitTauriEvent(page, "open_settings", target), target, count + 1)
  const window = await openLinkedSubWindow(page, opened, options)
  await expect(window.getByTestId("settings-window")).toBeVisible()
  return window
}

/**
 * Opens Settings at Tools (`target` names its focus and server) once the first read of the statuses of the tools
 * answered (`cli_tools_status`): it runs every CLI and its fakes, seconds under load, past the 5 s of an assertion of
 * what a row shows. A test of the read itself holds it (`HeldAnswer::CliTools`) and opens the window with
 * `openSettingsWindow`
 */
export async function openToolsSettings(page: Page, target: SettingsTarget = { page: "tools" }): Promise<Page> {
  let read!: Promise<unknown>
  const window = await openSettingsWindow(page, target, {
    beforeLoad: async window => {
      read = nextAnswer(window, "cli_tools_status")
    },
  })
  await read
  return window
}

/** The window gets the focus, which reads the statuses of the tools again; resolves once that read answered */
export async function focusAndReadTools(window: Page): Promise<void> {
  await runAndWaitForAnswer(window, "cli_tools_status", () => emitTauriEvent(window, "tauri://focus"))
}

/** The tab of a page in the sidebar */
export function settingsTab(window: Page, name: string): Locator {
  return window.getByRole("tablist", { name: "Settings pages" }).getByRole("tab", { name, exact: true })
}

/**
 * The window asks to close (`tauri://close-requested`, ⌘W) as the answer of the next `command` lands: in a task posted
 * as its body is read, which runs after the page took the answer (the microtasks of its promises) and before the
 * render of what they set, which React posts after it in the same task source (`MessageChannel`, run in order). The
 * moment a close of the user can come that no wait of a test reaches
 */
export async function closeAsTheAnswerLands(window: Page, command: string): Promise<void> {
  await window.evaluate(suffix => {
    const descriptor = Object.getOwnPropertyDescriptor(Response.prototype, "arrayBuffer")!
    const arrayBuffer = descriptor.value as (this: Response) => Promise<ArrayBuffer>
    Response.prototype.arrayBuffer = async function (this: Response) {
      const body = await arrayBuffer.call(this)
      if (this.url.endsWith(suffix)) {
        Object.defineProperty(Response.prototype, "arrayBuffer", descriptor)
        const channel = new MessageChannel()
        channel.port1.onmessage = () => {
          channel.port1.close()
          ;(window as unknown as { __e2eEmitTauriEvent: (event: string, payload: unknown, label: string | null) => void }).__e2eEmitTauriEvent(
            "tauri://close-requested",
            null,
            null,
          )
        }
        channel.port2.postMessage(null)
      }
      return body
    }
  }, `/invoke/${command}`)
}

/** The writes the window sends (`set_repository_config`), in their order */
export function recordWrites(window: Page): Array<Record<string, unknown>> {
  const writes: Array<Record<string, unknown>> = []
  window.on("request", request => {
    if (request.url().endsWith("/invoke/set_repository_config")) {
      // the order of each write of the page (`nextWriteOrder`: its page and its number, which grows with the order of the
      // requests) is the writer's; a test reads what was written
      const { order: _order, ...write } = request.postDataJSON() as Record<string, unknown>
      writes.push(write)
    }
  })
  return writes
}
