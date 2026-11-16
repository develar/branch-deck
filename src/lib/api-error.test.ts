import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { commandError, internalError } from "@/lib/api-error"
import type { ApiError } from "@/lib/bindings"

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
// the bindings import `Channel` too, which no command of this test sends
vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel: undefined }))

const { commands } = await import("@/lib/bindings")

const locked: ApiError = { code: "locked", message: "the repository is locked", details: null, retry: "same", operation_id: "01J" }

describe("commandError", () => {
  it("keeps the ApiError a command answered, as is", () => {
    expect(commandError(locked)).toBe(locked)
  })

  it("answers a refusal of Tauri itself (a string) as an internal error of its words", () => {
    expect(commandError("Command not found")).toEqual(internalError("Command not found"))
  })

  it("answers an Error of the page's IPC as an internal error of its message", () => {
    expect(commandError(new TypeError("Load failed"))).toEqual(internalError("Load failed"))
  })
})

// the catch of `typedError`, which `src-tauri/src/bindings_test.rs` writes into the bindings
describe("a command that answers a Result", () => {
  beforeEach(() => {
    invoke.mockReset()
  })

  it("resolves to its data", async () => {
    invoke.mockResolvedValue({ files: [] })
    await expect(commands.getUncommittedChanges({ repositoryPath: "/repo" })).resolves.toEqual({ status: "ok", data: { files: [] } })
    expect(invoke).toHaveBeenCalledWith("get_uncommitted_changes", { params: { repositoryPath: "/repo" } })
  })

  it("resolves to the ApiError it answered", async () => {
    invoke.mockRejectedValue(locked)
    await expect(commands.getUncommittedChanges({ repositoryPath: "/repo" })).resolves.toEqual({ status: "error", error: locked })
  })

  it("resolves a refusal of Tauri (a string: an unknown command, an ACL refusal, invalid arguments) to an internal error", async () => {
    invoke.mockRejectedValue("Command get_uncommitted_changes not found")
    await expect(commands.getUncommittedChanges({ repositoryPath: "/repo" })).resolves.toEqual({
      status: "error",
      error: internalError("Command get_uncommitted_changes not found"),
    })
  })

  it("resolves a failure of the IPC (an Error: a fetch of the E2E bridge, the Air shim) to an internal error, never a rejection", async () => {
    invoke.mockRejectedValue(new TypeError("Load failed"))
    await expect(commands.getUncommittedChanges({ repositoryPath: "/repo" })).resolves.toEqual({ status: "error", error: internalError("Load failed") })
  })
})
