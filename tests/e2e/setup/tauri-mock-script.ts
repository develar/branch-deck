import { fileURLToPath } from "node:url"
import { buildSync } from "esbuild"

let script: string | undefined

/**
 * The init script of every page of a test (`page.addInitScript(tauriMockInitScript())`): the page mock of
 * `tauri-mocks.ts` with the commands of app-api, bundled with what it imports (`@/platform/tauri-page`, which the page of
 * the IDE installs too), once per worker
 */
export function tauriMockInitScript(): { content: string } {
  script ??= buildSync({
    entryPoints: [fileURLToPath(new URL("tauri-mock-page.ts", import.meta.url))],
    bundle: true,
    write: false,
    format: "iife",
    target: "es2022",
    alias: { "@": fileURLToPath(new URL("../../../src", import.meta.url)) },
    logLevel: "error",
  }).outputFiles[0]!.text
  return { content: script }
}
