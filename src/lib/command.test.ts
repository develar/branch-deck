import { describe, expect, it, vi } from "vite-plus/test"

const { warnings, errors } = vi.hoisted(() => ({ warnings: [] as [string, unknown][], errors: [] as [string, unknown][] }))

vi.mock("@/lib/errors", () => ({
  getErrorDetails: (error: unknown) => (error instanceof Error ? error.message : String(error)),
  logWarning: (title: string, detail: unknown) => warnings.push([title, detail]),
  logError: (title: string, error: unknown) => errors.push([title, error]),
}))

const { readAhead } = await import("@/lib/command")

describe("readAhead", () => {
  it("answers whether it read; a failure of the command or of the IPC is a warning, never an error or a rejection", async () => {
    await expect(readAhead("the review of a branch", () => Promise.resolve({ status: "ok", data: null }))).resolves.toBe(true)
    expect(warnings).toEqual([])

    // the IPC failed (WebKit: a request its page no longer waited for), as the bindings rethrow an Error
    await expect(readAhead("the review of a branch", () => Promise.reject(new TypeError("Load failed")))).resolves.toBe(false)
    // the command answered its failure
    const gone = { code: "internal", message: "the branch is gone", details: null, retry: "no", operation_id: null } as const
    await expect(readAhead("the review of a branch", () => Promise.resolve({ status: "error", error: gone }))).resolves.toBe(false)

    expect(warnings).toEqual([
      ["Cannot read the review of a branch ahead", expect.objectContaining({ code: "internal", message: "Load failed" })],
      ["Cannot read the review of a branch ahead", gone],
    ])
    expect(errors).toEqual([])
  })
})
