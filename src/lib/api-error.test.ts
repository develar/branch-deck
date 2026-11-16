import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { commandError, internalError } from "@/lib/api-error"
import type { ApiError } from "@/lib/bindings"

const { invoke, warnToLog } = vi.hoisted(() => ({ invoke: vi.fn(), warnToLog: vi.fn(() => Promise.resolve()) }))
// the bindings import `Channel` too, which no command of this test sends
vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel: undefined }))
// the warning of the log of the app (`writeWarning`)
vi.mock("@tauri-apps/plugin-log", () => ({ warn: warnToLog }))

const { commands } = await import("@/lib/bindings")

const locked: ApiError = { code: "locked", message: "the repository is locked", details: null, retry: "same", operation_id: "01J" }

/** The invoke of Tauri: the command answers by `answer` */
function answerCommand(answer: () => Promise<unknown>): void {
  invoke.mockImplementation(answer)
}

describe("commandError", () => {
  it("keeps the ApiError a command answered, as is", () => {
    expect(commandError(locked, null)).toBe(locked)
  })

  it("answers a refusal of Tauri itself (a string) as an internal error of its words", () => {
    expect(commandError("Command not found", null)).toEqual(internalError("Command not found"))
  })

  it("answers an Error of the page's IPC as an internal error of its message", () => {
    expect(commandError(new TypeError("Load failed"), null)).toEqual(internalError("Load failed"))
  })

  it("answers a failure of no words with words of its own, never an empty message", () => {
    expect(commandError(new Error(""), null)).toEqual(internalError("The command failed without saying why"))
    expect(commandError("", null)).toEqual(internalError("The command failed without saying why"))
  })
})

// the catch of `typedError`, which `src-tauri/src/bindings_test.rs` writes into the bindings
describe("a command that answers a Result", () => {
  beforeEach(() => {
    invoke.mockReset()
    warnToLog.mockClear()
    vi.spyOn(console, "warn").mockImplementation(() => {})
    vi.spyOn(console, "error").mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it("resolves to its data", async () => {
    answerCommand(() => Promise.resolve({ files: [] }))
    await expect(commands.getUncommittedChanges({ repositoryPath: "/repo" })).resolves.toEqual({ status: "ok", data: { files: [] } })
    expect(invoke).toHaveBeenCalledWith("get_uncommitted_changes", { params: { repositoryPath: "/repo" } })
    expect(console.warn).not.toHaveBeenCalled()
    expect(warnToLog).not.toHaveBeenCalled()
  })

  it("resolves to the ApiError it answered, which the backend logged: no warning of the page", async () => {
    answerCommand(() => Promise.reject(locked))
    await expect(commands.getUncommittedChanges({ repositoryPath: "/repo" })).resolves.toEqual({ status: "error", error: locked })
    expect(console.warn).not.toHaveBeenCalled()
    expect(warnToLog).not.toHaveBeenCalled()
  })

  it("resolves a refusal of Tauri (a string: an unknown command, an ACL refusal, invalid arguments) to an internal error", async () => {
    answerCommand(() => Promise.reject("Command get_uncommitted_changes not found"))
    await expect(commands.getUncommittedChanges({ repositoryPath: "/repo" })).resolves.toEqual({
      status: "error",
      error: internalError("Command get_uncommitted_changes not found"),
    })
    // the backend logged nothing of it: one warning of the log names the command
    expect(console.warn).toHaveBeenCalledOnce()
    expect(console.warn).toHaveBeenCalledWith("Cannot run get_uncommitted_changes", "Command get_uncommitted_changes not found")
    expect(warnToLog).toHaveBeenCalledOnce()
    expect(warnToLog).toHaveBeenCalledWith("Cannot run get_uncommitted_changes", { keyValues: { detail: "Command get_uncommitted_changes not found" } })
  })

  it("resolves a failure of the IPC (an Error: a fetch of the E2E bridge, the Air shim) to an internal error, never a rejection", async () => {
    const failure = new TypeError("Load failed")
    answerCommand(() => Promise.reject(failure))
    await expect(commands.getUncommittedChanges({ repositoryPath: "/repo" })).resolves.toEqual({ status: "error", error: internalError("Load failed") })
    expect(console.warn).toHaveBeenCalledOnce()
    expect(console.warn).toHaveBeenCalledWith("Cannot run get_uncommitted_changes", failure)
    expect(warnToLog).toHaveBeenCalledOnce()
    expect(warnToLog).toHaveBeenCalledWith("Cannot run get_uncommitted_changes", { keyValues: { detail: "Load failed" } })
    // the warning reached the log of the app
    expect(console.error).not.toHaveBeenCalled()
  })
})
