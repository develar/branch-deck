import { error as logErrorToBackend } from "@tauri-apps/plugin-log"
import { describe, expect, it, vi } from "vite-plus/test"
import { commandFailure, getErrorDetails, getErrorOperationId, logError } from "@/lib/errors"

vi.mock("@tauri-apps/plugin-log", () => ({ error: vi.fn(async () => {}) }))

/** The page's own sources, without the tests and the generated bindings */
const sources = import.meta.glob<string>(["/src/**/*.{ts,tsx}", "!/src/**/*.test.{ts,tsx}", "!/src/lib/bindings.ts"], { eager: true, query: "?raw", import: "default" })

describe("getErrorDetails", () => {
  it('returns the message of an Error, without the "Error:" prefix of its toString', () => {
    expect(getErrorDetails(new Error("Repository not accessible: /work/app"))).toBe("Repository not accessible: /work/app")
  })

  it("returns the message of a subclass, without its name", () => {
    expect(getErrorDetails(new TypeError("fetch failed"))).toBe("fetch failed")
  })

  it("returns the message of a CommandError returned by a Tauri command", () => {
    expect(getErrorDetails({ message: "Repository not accessible: /work/app" })).toBe("Repository not accessible: /work/app")
  })

  it("stringifies anything else", () => {
    expect(getErrorDetails("offline")).toBe("offline")
    expect(getErrorDetails(42)).toBe("42")
  })
})

describe("logError", () => {
  it("passes the operation a CommandError names to the backend log line, as operation_id", () => {
    vi.spyOn(console, "error").mockImplementation(() => {})
    logError("Cannot sync", { message: "merge-tree failed", kind: null, operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })
    expect(logErrorToBackend).toHaveBeenLastCalledWith("Cannot sync", {
      keyValues: { error: "merge-tree failed", operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" },
    })

    logError("Cannot open", new Error("closed"))
    expect(logErrorToBackend).toHaveBeenLastCalledWith("Cannot open", { keyValues: { error: "closed", operation_id: undefined } })
  })
})

describe("getErrorOperationId", () => {
  it("returns the operation a CommandError names, and nothing for any other error", () => {
    expect(getErrorOperationId({ message: "rejected", kind: null, operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })).toBe("01K8ZQ3V6Y4M2N7P9R1S5T0W2X")
    expect(getErrorOperationId({ message: "rejected", kind: null, operation_id: null })).toBeUndefined()
    expect(getErrorOperationId(new Error("closed"))).toBeUndefined()
    expect(getErrorOperationId("offline")).toBeUndefined()
  })
})

describe("commandFailure", () => {
  it("throws what a command returned with its message, its kind and the operation it names", () => {
    const failure = commandFailure({ message: "rejected", kind: "lockWaitCanceled", operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })
    expect(failure).toBeInstanceOf(Error)
    expect(getErrorDetails(failure)).toBe("rejected")
    expect(failure).toMatchObject({ kind: "lockWaitCanceled" })
    expect(getErrorOperationId(failure)).toBe("01K8ZQ3V6Y4M2N7P9R1S5T0W2X")
    expect(getErrorOperationId(commandFailure({ message: "rejected" }))).toBeUndefined()
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
