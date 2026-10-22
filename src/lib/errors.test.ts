import { describe, expect, it, vi } from "vite-plus/test"
import { getErrorDetails, getErrorOperationId } from "@/lib/errors"

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))

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

describe("getErrorOperationId", () => {
  it("returns the operation a CommandError names, and nothing for any other error", () => {
    expect(getErrorOperationId({ message: "rejected", kind: null, operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })).toBe("01K8ZQ3V6Y4M2N7P9R1S5T0W2X")
    expect(getErrorOperationId({ message: "rejected", kind: null, operation_id: null })).toBeUndefined()
    expect(getErrorOperationId(new Error("closed"))).toBeUndefined()
    expect(getErrorOperationId("offline")).toBeUndefined()
  })
})
