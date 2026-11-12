import { readFile } from "node:fs/promises"
import { extname, join, normalize } from "node:path"
import { fileURLToPath } from "node:url"
import type { FrameLocator, Page } from "@playwright/test"
import { buildSync } from "esbuild"
import { TEST_SERVER_PORT, TEST_SERVER_URL } from "../helpers/test-server-url"
import type { AirHostConfig, HostCall, HostContextTheme, HostView } from "./host-page"
import darkTheme from "./theme-dark.json" with { type: "json" }
import lightTheme from "./theme-light.json" with { type: "json" }

/** The origin of the frames of Branch Deck, as the IDE gives each app its own (`<id>.air-genui-app.local`) */
export const APP_ORIGIN = "https://branch-deck.air-genui-app.local"

/** The themes of the IDE as its host context names them (`styles.variables` of SEP-1865 and the accent of Air) */
export const IDE_THEMES = { light: lightTheme as HostContextTheme, dark: darkTheme as HostContextTheme }

/**
 * The WebSocket of the app link on the test server: `brd mcp` (`mcp-host`) on the fixture repository `repoId`, as the
 * IDE starts it for a project
 */
export function appLinkUrl(repoId: string): string {
  return `ws://127.0.0.1:${TEST_SERVER_PORT}/repositories/${repoId}/mcp`
}

const DIST_AIR = fileURLToPath(new URL("../../../dist-air/", import.meta.url))

const CONTENT_TYPES: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".woff2": "font/woff2",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".json": "application/json",
}

let hostScript: string | undefined

/** The script of the host page, bundled once per worker */
function hostPageScript(): string {
  hostScript ??= buildSync({
    entryPoints: [fileURLToPath(new URL("host-page.ts", import.meta.url))],
    bundle: true,
    write: false,
    format: "iife",
    target: "es2022",
    logLevel: "error",
  }).outputFiles[0]!.text
  return hostScript
}

/**
 * Serves the files of the Air build at the origin of the app, as `brd mcp` serves them by `resources/read`: a path with
 * no file is the page of a view (`index.html`)
 */
async function serveAppFiles(page: Page): Promise<void> {
  await page.route(`${APP_ORIGIN}/**`, async route => {
    const path = normalize(decodeURIComponent(new URL(route.request().url()).pathname)).replace(/^\/+/, "")
    const file = extname(path) === "" || path.startsWith("..") ? "index.html" : path
    try {
      const body = await readFile(join(DIST_AIR, file))
      await route.fulfill({ status: 200, body, contentType: CONTENT_TYPES[extname(file)] ?? "application/octet-stream" })
    } catch {
      await route.fulfill({ status: 404, body: `No ${file} in dist-air` })
    }
  })
}

/** The fake IDE of a test: the views it shows, what `brd mcp` asked of it, and the frame of a view */
export class FakeIde {
  constructor(readonly page: Page) {}

  /** The frame of the view `instance` (`main:1`, `settings:1`) */
  frame(instance: string): FrameLocator {
    return this.page.frameLocator(`iframe[data-instance="${instance}"]`)
  }

  views(): Promise<HostView[]> {
    return this.page.evaluate(() => (window as unknown as { __airHost: { views(): HostView[] } }).__airHost.views())
  }

  /** The requests of `brd mcp` to the IDE and the host tools the pages called, in order */
  calls(): Promise<HostCall[]> {
    return this.page.evaluate(() => (window as unknown as { __airHost: { calls(): HostCall[] } }).__airHost.calls())
  }

  setTheme(theme: HostContextTheme): Promise<void> {
    return this.page.evaluate(next => (window as unknown as { __airHost: { setTheme(theme: HostContextTheme): void } }).__airHost.setTheme(next), theme)
  }

  closeView(instance: string): Promise<void> {
    return this.page.evaluate(it => (window as unknown as { __airHost: { close(instance: string): Promise<void> } }).__airHost.close(it), instance)
  }

  /** The IDE opens the view `view` with `params` (the place of the Settings window, `{data: {page: "tools"}}`): its instance */
  openView(view: string, params: Record<string, unknown> = {}): Promise<string> {
    return this.page.evaluate(
      ([it, given]) => (window as unknown as { __airHost: { open(view: string, params: Record<string, unknown>): Promise<string> } }).__airHost.open(it, given),
      [view, params] as const,
    )
  }

  /** A click on an action of the app file in the title of the tool window: the IDE calls its tool with the origin action */
  runAction(action: string, tool: string): Promise<unknown> {
    return this.page.evaluate(([it, name]) => (window as unknown as { __airHost: { runAction(action: string, tool: string): Promise<unknown> } }).__airHost.runAction(it, name), [
      action,
      tool,
    ] as const)
  }
}

/**
 * Opens the fake IDE on the fixture repository `repoId` at `repositoryPath` with the main view: the host page is a page
 * of the test server's origin, so its WebSocket to the app link is a loopback one, and each frame loads the Air build
 */
export async function openFakeIde(
  page: Page,
  { repoId, repositoryPath, theme = IDE_THEMES.light, appUrl = appLinkUrl(repoId) }: { repoId: string; repositoryPath: string; theme?: HostContextTheme; appUrl?: string },
): Promise<FakeIde> {
  await serveAppFiles(page)
  // a document of the server's origin, its content the host page's
  await page.goto(`${TEST_SERVER_URL}/health`)
  const config: AirHostConfig = { appUrl, appOrigin: APP_ORIGIN, repositoryPath, theme }
  await page.setContent(`<!doctype html><html><head><title>Fake IDE</title></head><body style="margin: 0"></body></html>`)
  await page.evaluate(value => {
    ;(window as unknown as { __airHostConfig: AirHostConfig }).__airHostConfig = value
  }, config)
  await page.addScriptTag({ content: hostPageScript() })
  await page.evaluate(async () => {
    const host = (window as unknown as { __airHost: { ready: Promise<void>; open(view: string): Promise<string> } }).__airHost
    await host.ready
    await host.open("main")
  })
  return new FakeIde(page)
}
