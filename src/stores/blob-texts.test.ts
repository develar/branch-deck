// @spec spec/commit/commit-files.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { BlobRequest, BlobText } from "@/lib/bindings"
import { BLOB_TEXT_BATCH, blobTextsStore, MAX_BLOB_TEXT_LENGTH, requestBlobTexts } from "@/stores/blob-texts"

const getBlobTexts = vi.hoisted(() => vi.fn())

vi.mock("@/lib/errors", () => ({ logError: () => {} }))
vi.mock("@/lib/command", () => import("@/test-support/invoke-command-mock"))
vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))
vi.mock("@/lib/bindings", () => ({ commands: { getBlobTexts } }))

let length = 4
const answer = async ({ blobs }: { blobs: BlobRequest[] }) => ({ status: "ok", data: blobs.map(({ id }): BlobText => ({ kind: "text", id, text: "x".repeat(length) })) })

beforeEach(() => {
  blobTextsStore.setState({ byId: new Map() })
  length = 4
  getBlobTexts.mockReset()
  getBlobTexts.mockImplementation(answer)
})

function calls(): string[][] {
  return getBlobTexts.mock.calls.map(([params]) => (params as { blobs: BlobRequest[] }).blobs.map(blob => blob.id))
}

async function settle() {
  // the requests of the task go out in a microtask: before it, nothing is loading yet
  await new Promise(resolve => setTimeout(resolve, 0))
  await vi.waitFor(() => {
    expect([...blobTextsStore.getState().byId.values()].some(entry => entry.kind === "loading")).toBe(false)
  })
}

describe("requestBlobTexts", () => {
  it("reads the ids of one task in one call, and each id once", async () => {
    requestBlobTexts(["a", "b"])
    requestBlobTexts(["b", "c"])
    await settle()
    expect(calls()).toEqual([["a", "b", "c"]])
    requestBlobTexts(["a", "c"])
    await settle()
    expect(calls()).toHaveLength(1)
    expect(blobTextsStore.getState().byId.get("c")).toEqual({ kind: "loaded", blob: { kind: "text", id: "c", text: "xxxx" } })
  })

  it("reads many ids in batches", async () => {
    requestBlobTexts(Array.from({ length: BLOB_TEXT_BATCH + 1 }, (_, index) => `id${index}`))
    await settle()
    expect(calls().map(ids => ids.length)).toEqual([BLOB_TEXT_BATCH, 1])
  })

  it("reads the batches of a large Find one after another, at most one call in flight", async () => {
    let inFlight = 0
    let most = 0
    getBlobTexts.mockImplementation(async (params: { blobs: BlobRequest[] }) => {
      inFlight++
      most = Math.max(most, inFlight)
      await new Promise(resolve => setTimeout(resolve, 1))
      inFlight--
      return answer(params)
    })
    const ids = Array.from({ length: 200 }, (_, index) => `id${index}`)
    requestBlobTexts(ids)
    // a second task while the first reads: its batch waits in the queue too
    await new Promise(resolve => setTimeout(resolve, 0))
    requestBlobTexts(["late"])
    await settle()
    expect(most).toBe(1)
    expect(calls().flat()).toEqual([...ids, "late"])
    expect(calls().map(batch => batch.length)).toEqual([BLOB_TEXT_BATCH, BLOB_TEXT_BATCH, BLOB_TEXT_BATCH, 200 - 3 * BLOB_TEXT_BATCH, 1])
  })

  it("keeps a failed read as an error and reads it again on the next request", async () => {
    getBlobTexts.mockImplementationOnce(async () => ({ status: "error", error: { message: "gone" } }))
    requestBlobTexts(["a"])
    await settle()
    expect(blobTextsStore.getState().byId.get("a")).toEqual({ kind: "error", message: "gone" })
    requestBlobTexts(["a"])
    await settle()
    expect(blobTextsStore.getState().byId.get("a")?.kind).toBe("loaded")
  })

  it("drops the least recently requested texts over the length, never the ones just read", async () => {
    length = MAX_BLOB_TEXT_LENGTH / 2
    requestBlobTexts(["a"])
    await settle()
    requestBlobTexts(["b"])
    await settle()
    // a was used again: b goes first
    requestBlobTexts(["a"])
    requestBlobTexts(["c"])
    await settle()
    expect([...blobTextsStore.getState().byId.keys()]).toEqual(["a", "c"])
  })
})
