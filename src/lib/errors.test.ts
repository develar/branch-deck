import { error as logErrorToBackend } from "@tauri-apps/plugin-log"
import { describe, expect, it, vi } from "vite-plus/test"
import { toast } from "@/components/ui/toast"
import { internalError } from "@/lib/api-error"
import type { ApiError, LockHolderInfo } from "@/lib/bindings"
import { commandFailure, describeError, getErrorCode, getErrorDetails, getErrorOperationId, isApiError, logError, notifyError } from "@/lib/errors"

vi.mock("@tauri-apps/plugin-log", () => ({ error: vi.fn(async () => {}) }))
vi.mock("@/components/ui/toast", () => ({ toast: { add: vi.fn(), close: vi.fn() } }))

/** The `locked` error of a command, as `app_api::api_error` answers it */
function lockedBy(holder: LockHolderInfo | null): ApiError {
  return {
    code: "locked",
    message: "`bd push auth` (pid 4242) is changing this repository; try again when it is done",
    details: { type: "locked", holder },
    retry: "same",
    operation_id: null,
  }
}

const BD_PUSH: LockHolderInfo = { tool: "bd", command: "bd push auth", pid: 4242, held_for: 40 }

/** The `ApiError` of each case of `app_api::api_error::contract_cases`, as the app and the e2e server answer it */
const CONTRACT = Object.values(import.meta.glob<Record<string, ApiError>>("/crates/app-api/fixtures/api_errors.json", { eager: true, import: "default" }))[0]

/** The page's own sources, without the tests and the generated bindings */
const sources = import.meta.glob<string>(["/src/**/*.{ts,tsx}", "!/src/**/*.test.{ts,tsx}", "!/src/lib/bindings.ts"], { eager: true, query: "?raw", import: "default" })

describe("getErrorDetails", () => {
  it('returns the message of an Error, without the "Error:" prefix of its toString', () => {
    expect(getErrorDetails(new Error("Repository not accessible: /work/app"))).toBe("Repository not accessible: /work/app")
  })

  it("returns the message of a subclass, without its name", () => {
    expect(getErrorDetails(new TypeError("fetch failed"))).toBe("fetch failed")
  })

  it("returns the message of an ApiError returned by a Tauri command", () => {
    expect(getErrorDetails(internalError("Repository not accessible: /work/app"))).toBe("Repository not accessible: /work/app")
  })

  it("stringifies anything else", () => {
    expect(getErrorDetails("offline")).toBe("offline")
    expect(getErrorDetails(42)).toBe("42")
  })
})

describe("logError", () => {
  it("passes the operation an ApiError names to the backend log line, as operation_id", () => {
    vi.spyOn(console, "error").mockImplementation(() => {})
    logError("Cannot sync", { ...internalError("merge-tree failed"), operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })
    expect(logErrorToBackend).toHaveBeenLastCalledWith("Cannot sync", {
      keyValues: { error: "merge-tree failed", operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" },
    })

    logError("Cannot open", new Error("closed"))
    expect(logErrorToBackend).toHaveBeenLastCalledWith("Cannot open", { keyValues: { error: "closed", operation_id: undefined } })
  })
})

describe("getErrorOperationId", () => {
  it("returns the operation an ApiError names, and nothing for any other error", () => {
    expect(getErrorOperationId({ ...internalError("rejected"), operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })).toBe("01K8ZQ3V6Y4M2N7P9R1S5T0W2X")
    expect(getErrorOperationId(internalError("rejected"))).toBeUndefined()
    expect(getErrorOperationId(new Error("closed"))).toBeUndefined()
    expect(getErrorOperationId("offline")).toBeUndefined()
  })
})

describe("commandFailure", () => {
  it("throws what a command returned with its message, its code and the operation it names", () => {
    const failure = commandFailure({ ...internalError("rejected"), code: "canceled", operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })
    expect(failure).toBeInstanceOf(Error)
    expect(getErrorDetails(failure)).toBe("rejected")
    expect(failure).toMatchObject({ code: "canceled" })
    expect(getErrorOperationId(failure)).toBe("01K8ZQ3V6Y4M2N7P9R1S5T0W2X")
    expect(getErrorOperationId(commandFailure(internalError("rejected")))).toBeUndefined()
  })

  it("is what the page throws for a failed command: no new Error of its message drops the operation", () => {
    expect(Object.keys(sources).length, "the sources the scan reads").toBeGreaterThan(100)
    const dropped = Object.entries(sources).flatMap(([file, text]) =>
      text
        .split("\n")
        .map((line, index) => ({ line, at: `${file}:${index + 1}` }))
        // an Error made of the message, or the message alone handed to what tells the user
        .filter(({ line }) => /new Error\([^)]*\.error\.message/.test(line) || /(notifyError|recordError|logError|feedback\.error)\([^)]*\.error\.message\)/.test(line))
        .map(({ at }) => at),
    )
    expect(dropped).toEqual([])
  })
})

describe("isApiError and getErrorCode", () => {
  it("tell the ApiError of a command by its code from an Error of the page", () => {
    expect(isApiError(lockedBy(null))).toBe(true)
    expect(getErrorCode(lockedBy(null))).toBe("locked")
    expect(getErrorCode(internalError("merge-tree failed"))).toBe("internal")
    expect(isApiError(new Error("closed"))).toBe(false)
    expect(getErrorCode(new Error("closed"))).toBeUndefined()
    expect(getErrorCode({ message: "canceled" })).toBeUndefined()
    expect(getErrorCode("offline")).toBeUndefined()
  })
})

describe("describeError", () => {
  it("names the holder of a held repository lock and how long it has held it, as the lock record says it", () => {
    expect(describeError(lockedBy(BD_PUSH))).toBe("`bd push auth` (pid 4242) has held the repository for 40 s")
    expect(describeError(lockedBy({ ...BD_PUSH, held_for: 723 }))).toBe("`bd push auth` (pid 4242) has held the repository for 12 min 3 s")
    expect(describeError(lockedBy({ tool: "app", command: "branch-deck", pid: 7, held_for: 5 }))).toBe("Branch Deck (pid 7) has held the repository for 5 s")
  })

  it("says another operation holds the repository when the holder left no record, never the lock file", () => {
    expect(describeError(lockedBy(null))).toBe("Another Branch Deck operation holds the repository")
  })

  it("keeps the words of a lock file of git, which name the file and say to remove a stale one, never another Branch Deck operation", () => {
    const refLocked = CONTRACT?.ref_locked
    expect(refLocked?.code).toBe("locked")
    expect(describeError(refLocked)).toBe(refLocked?.message)
    expect(describeError(refLocked)).toContain("/repo/.git/refs/heads/master.lock exists")
  })

  it("is the message of any other error", () => {
    expect(describeError({ ...internalError("The branch moved"), code: "head_moved", retry: "after_reread" })).toBe("The branch moved")
    expect(describeError(new Error("closed"))).toBe("closed")
  })
})

describe("notifyError", () => {
  it("toasts a held repository lock with its holder, without the holder the other operation, and a plain Error with its message", () => {
    vi.spyOn(console, "error").mockImplementation(() => {})
    const shown = () => vi.mocked(toast.add).mock.lastCall?.[0]

    notifyError("Cannot update the project", lockedBy(BD_PUSH))
    expect(shown()).toMatchObject({ type: "error", title: "Cannot update the project", description: "`bd push auth` (pid 4242) has held the repository for 40 s" })
    // the log keeps the backend's words
    expect(logErrorToBackend).toHaveBeenLastCalledWith("Cannot update the project", {
      keyValues: { error: "`bd push auth` (pid 4242) is changing this repository; try again when it is done", operation_id: undefined },
    })

    // refused again by the same holder, a while later: its toast changes in place
    const first = shown()?.id
    notifyError("Cannot update the project", lockedBy({ ...BD_PUSH, held_for: 75 }))
    expect(shown()).toMatchObject({ id: first, description: "`bd push auth` (pid 4242) has held the repository for 1 min 15 s" })

    notifyError("Cannot update the project", lockedBy(null))
    expect(shown()).toMatchObject({ description: "Another Branch Deck operation holds the repository" })

    notifyError("Cannot open", new Error("closed"))
    expect(shown()).toMatchObject({ title: "Cannot open", description: "closed" })
  })
})
