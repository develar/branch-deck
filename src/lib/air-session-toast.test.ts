import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { followAirSession } from "@/lib/air-session-toast"
import type { AirSessionStatus, AirWatchEnd } from "@/lib/bindings"

const { toast, watch } = vi.hoisted(() => ({
  toast: { add: vi.fn(), update: vi.fn() },
  watch: { session: vi.fn(), stop: vi.fn() },
}))
vi.mock("@/components/ui/toast", () => ({ toast }))
vi.mock("@/lib/air-launch", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/air-launch")>()),
  watchAirSession: watch.session,
  stopAirWatch: watch.stop,
}))

/** A watch whose statuses and end the test sends */
function heldWatch() {
  let send: (status: AirSessionStatus) => void = () => {}
  let end: (end: AirWatchEnd) => void = () => {}
  watch.session.mockImplementation((_project: string, _sessionId: string, onStatus: (status: AirSessionStatus) => void) => {
    send = onStatus
    return new Promise<AirWatchEnd>(resolve => {
      end = resolve
    })
  })
  return { send: (status: AirSessionStatus) => send(status), end: (answer: AirWatchEnd) => end(answer) }
}

function lastUpdate(): Record<string, unknown> {
  return toast.update.mock.calls.at(-1)![1] as Record<string, unknown>
}

describe("followAirSession", () => {
  beforeEach(() => {
    toast.add.mockReset().mockReturnValue("toast-1")
    toast.update.mockReset()
    watch.session.mockReset()
    watch.stop.mockReset()
  })

  it("says it started, follows each status in place, and stops the watch once the session is done", async () => {
    const held = heldWatch()
    followAirSession("/work/idea", "s-1", "Codex", "Fix the Safe Push of auth · IU-263.1")
    expect(toast.add).toHaveBeenCalledWith(expect.objectContaining({ type: "loading", title: "Started Codex in Air", timeout: 0 }))
    expect(watch.session).toHaveBeenCalledWith("/work/idea", "s-1", expect.any(Function))

    held.send({ title: "Fix auth", activity: "needs_input" })
    expect(lastUpdate()).toEqual({ type: "info", title: "Codex waits for you in Air", description: "Fix auth" })
    expect(watch.stop).not.toHaveBeenCalled()

    held.send({ title: null, activity: "unread" })
    expect(lastUpdate()).toMatchObject({ type: "success", title: "Codex is done in Air", description: "Fix the Safe Push of auth · IU-263.1", timeout: 5_000 })
    expect(watch.stop).toHaveBeenCalledWith("s-1")

    // the stopped watch says nothing more
    const updates = toast.update.mock.calls.length
    held.end({ status: "stopped" })
    await Promise.resolve()
    expect(toast.update).toHaveBeenCalledTimes(updates)
  })

  it("says the session is gone, and a close of the toast stops the watch", async () => {
    const held = heldWatch()
    followAirSession("/work/idea", "s-1", "Codex", "d")
    held.send({ title: null, activity: "processing" })
    held.end({ status: "refused", code: "NO_SESSION", message: null })
    await vi.waitFor(() => expect(lastUpdate()).toMatchObject({ type: "info", title: "The session of Codex is gone in Air", description: "d" }))

    heldWatch()
    followAirSession("/work/idea", "s-2", "Codex", "d")
    const { onClose } = toast.add.mock.calls.at(-1)![0] as { onClose: () => void }
    onClose()
    expect(watch.stop).toHaveBeenCalledWith("s-2")
  })
})
