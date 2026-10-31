// @spec spec/repository/title-bar.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { checkForUpdatesFromMenu } from "@/hooks/useAutoUpdate"
import type { UpdateInfo } from "@/lib/bindings"
import { openToasts, resetToasts } from "@/test-support/toast-mock"

const checkForUpdates = vi.hoisted(() => vi.fn())
const getUpdateStatus = vi.hoisted(() => vi.fn())

vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)
vi.mock("@/lib/bindings", () => ({ commands: { checkForUpdates, getUpdateStatus } }))
vi.mock("@/lib/errors", () => ({ logError: () => {}, notifyError: () => {} }))
vi.mock("@/stores/setup", () => ({ refreshSetup: () => Promise.resolve() }))
vi.mock("@tauri-apps/api/app", () => ({ getName: () => Promise.resolve("BranchDeck") }))
vi.mock("@/hooks/useAppCommand", () => ({ useAppCommand: () => {} }))

function info(status: UpdateInfo["status"]): UpdateInfo {
  return { current_version: "1.2.0", available_version: "1.3.0", is_update_available: true, status }
}

beforeEach(() => {
  resetToasts()
  checkForUpdates.mockReset()
  getUpdateStatus.mockReset()
})

describe("Check for Updates… of the menu", () => {
  it("says at once that the silent check running meanwhile downloads its version, then offers it", async () => {
    let answer = (_info: UpdateInfo) => {}
    checkForUpdates.mockReturnValue(new Promise(resolve => (answer = info => resolve({ status: "ok", data: info }))))
    getUpdateStatus.mockResolvedValue({ status: "ok", data: info("Downloading") })

    checkForUpdatesFromMenu()
    await vi.waitFor(() => expect(openToasts().map(toast => toast.title)).toEqual(["Downloading version 1.3.0…"]))

    answer(info("Downloaded"))
    await vi.waitFor(() => expect(openToasts().map(toast => toast.title)).toEqual(["Update available"]))
  })
})
