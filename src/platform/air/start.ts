/**
 * The start of the page of a view of the IDE (`src/air-main.ts`): `ui/initialize` on the frame link with the extension
 * `x-air-app`, the theme of the host context, `bd_bootstrap` of the view, the globals of its init script and the
 * internals of Tauri; then the usual main runs on the path of the page of the view
 */
import { applyBootstrap, pagePathOf } from "@/platform/air/bootstrap"
import type { Bootstrap } from "@/platform/air/bootstrap"
import { FrameLink, parentEndpoint } from "@/platform/air/frame-link"
import type { FrameEndpoint } from "@/platform/air/frame-link"
import { AirPage, BD_BOOTSTRAP } from "@/platform/air/shim"
import type { AirViewContext } from "@/platform/air/shim"
import { AirTheme } from "@/platform/air/theme"
import type { HostTheme } from "@/platform/air/theme"
import type { PageCommand } from "@/platform/tauri-page"

/** The version of MCP Apps (SEP-1865) the page speaks, and of the extension of Air */
export const MCP_APPS_VERSION = "2026-01-26"
export const X_AIR_APP_VERSION = "1.0.0"

/** The commands of app-api (`crates/app-api/commands.json`): which argument of a command gets its events */
const PAGE_COMMANDS = Object.values(import.meta.glob<PageCommand[]>("/crates/app-api/commands.json", { eager: true, import: "default" }))[0] ?? []

/** The answer of `ui/initialize` the page reads */
interface InitializeResult {
  readonly hostContext?: HostTheme & { readonly "x-air-app"?: AirViewContext }
}

/** Starts the page of the view the host names on `target`; resolves once the page may run its main */
export async function startAirPage(target: Window, endpoint: FrameEndpoint = parentEndpoint(target)): Promise<AirPage> {
  const link = new FrameLink(endpoint)
  const theme = new AirTheme(target)
  link.on("ui/notifications/host-context-changed", params => theme.apply(params as HostTheme))
  const initialized = await link.request<InitializeResult>("ui/initialize", {
    protocolVersion: MCP_APPS_VERSION,
    appInfo: { name: "branch-deck", version: X_AIR_APP_VERSION },
    appCapabilities: { experimental: { "x-air-app": { version: X_AIR_APP_VERSION } } },
  })
  const context = initialized.hostContext?.["x-air-app"]
  if (context === undefined) {
    throw new Error("The host serves no x-air-app: Branch Deck runs only in a view of the IDE")
  }
  theme.apply(initialized.hostContext ?? {})
  link.notify("ui/notifications/initialized")

  const page = new AirPage(link, context, PAGE_COMMANDS)
  const bootstrap = await page.callTool(BD_BOOTSTRAP, { view: context.view })
  if (bootstrap.isError === true) {
    throw new Error(`Cannot start the view ${context.view}: ${JSON.stringify(bootstrap.structuredContent?.error ?? bootstrap.content)}`)
  }
  applyBootstrap(target, (bootstrap.structuredContent ?? {}) as Bootstrap)
  page.install(target)
  // the router reads the page of the window from its path (`loadWindowPage`)
  const path = pagePathOf(context.view)
  if (target.location.pathname !== path) {
    target.history.replaceState(target.history.state, "", `${path}${target.location.search}${target.location.hash}`)
  }
  return page
}
