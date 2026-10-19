// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import { BackendSyntax } from "@/lib/diff/highlight/backend-syntax"
import type { BlobSyntaxParams } from "@/lib/diff/highlight/backend-syntax"
import { HighlightClient } from "@/lib/diff/highlight/client"
import type { BlobWant, HighlightPort } from "@/lib/diff/highlight/client"
import { PAGE_LINES } from "@/lib/diff/highlight/protocol"
import type { HighlightRequest } from "@/lib/diff/highlight/protocol"
import { TokenCache } from "@/lib/diff/highlight/token-cache"

const OBJECT_ID = "a".repeat(40)

/** An answer of the backend: the header, then the line starts and the runs of the lines `[from, to)` */
function answer(status: number, lineCount: number, from: number, to: number, runsOfLine: (line: number) => number[] = () => []): ArrayBuffer {
  const starts: number[] = [0]
  const runs: number[] = []
  for (let line = from; line < to; line++) {
    runs.push(...runsOfLine(line))
    starts.push(runs.length / 2)
  }
  return new Uint32Array([1, status, lineCount, from, to, runs.length / 2, ...(status === 0 ? starts : []), ...runs]).buffer
}

/** A backend that records the calls and answers each with `respond` */
function fakeBackend(respond: (params: BlobSyntaxParams) => ArrayBuffer) {
  const calls: BlobSyntaxParams[] = []
  const call = (params: BlobSyntaxParams) => {
    calls.push(params)
    return Promise.resolve(respond(params))
  }
  return { calls, call }
}

function want(blobId: string, pages: number[], extra: Partial<BlobWant> = {}): BlobWant {
  return { blobId, language: "kotlin", text: "val a = 1\n", pages, priority: 0, filePath: "A.kt", worktreePath: null, group: "A.kt", ...extra }
}

/** A worker port that records the requests */
function recordingPort(): HighlightPort & { requests: HighlightRequest[] } {
  const requests: HighlightRequest[] = []
  return { requests, post: request => requests.push(request), listen: () => {} }
}

async function settle(): Promise<void> {
  for (let index = 0; index < 5; index++) {
    await Promise.resolve()
  }
}

function setUp(respond: (params: BlobSyntaxParams) => ArrayBuffer) {
  const cache = new TokenCache()
  const port = recordingPort()
  const client = new HighlightClient([port], cache)
  const backend = fakeBackend(respond)
  client.useBackend(
    new BackendSyntax(
      cache,
      () => "/repo",
      group => client.fallBack(group),
      backend.call,
    ),
  )
  return { cache, port, client, calls: backend.calls }
}

describe("BackendSyntax", () => {
  it("asks the backend for a run of pages and the next one, and keeps its answer as exact pages", async () => {
    const lineCount = PAGE_LINES * 2 + 10
    const { cache, port, client, calls } = setUp(params => answer(0, lineCount, params.fromLine, Math.min(params.toLine, lineCount), line => (line % 2 === 0 ? [0, 7] : [])))
    client.want("v", [want(OBJECT_ID, [0])])
    await settle()
    expect(calls).toEqual([{ repositoryPath: "/repo", blob: { id: OBJECT_ID, path: null }, filePath: "A.kt", fromLine: 0, toLine: PAGE_LINES * 2, text: null }])
    // the worker is not asked for a Kotlin blob
    expect(port.requests.filter(request => request.type === "text")).toEqual([])
    const first = cache.get(OBJECT_ID, 0)!
    expect([first.exact, first.lineStarts.length, first.lineStarts[1], first.lineStarts[2], Array.from(first.runs.subarray(0, 2))]).toEqual([true, PAGE_LINES + 1, 1, 1, [0, 7]])
    expect(cache.get(OBJECT_ID, 1)?.lineStarts.length).toBe(PAGE_LINES + 1)
    // the pages it has are not asked for again
    client.want("v", [want(OBJECT_ID, [0, 1])])
    await settle()
    expect(calls).toHaveLength(1)
  })

  it("sends the text of a blob the backend lacks, and of one the view made, with the first call", async () => {
    const { cache, client, calls } = setUp(params => (params.text === null ? answer(2, 0, 0, 0) : answer(0, 2, 0, 2, () => [4, 1])))
    client.want("v", [want(OBJECT_ID, [0]), want("merge-result-1", [0], { group: "M.kt" })])
    await settle()
    await settle()
    expect(calls.map(call => [call.blob.id, call.text !== null])).toEqual([
      [OBJECT_ID, false],
      ["merge-result-1", true],
      [OBJECT_ID, true],
    ])
    expect([cache.get(OBJECT_ID, 0)?.exact, cache.get("merge-result-1", 0)?.exact]).toEqual([true, true])
  })

  it("colours both sides of a file the backend cannot colour in the worker", async () => {
    const { port, client, calls } = setUp(() => answer(1, 0, 0, 0))
    client.want("v", [want(OBJECT_ID, [0]), want("b".repeat(40), [0])])
    await settle()
    // the first answer sends the file to the worker: the other side too, and the backend is not asked again
    const texts = port.requests.filter(request => request.type === "text").map(request => request.blobId)
    expect(texts).toEqual([OBJECT_ID, "b".repeat(40)])
    client.want("v", [want(OBJECT_ID, [0, 1]), want("b".repeat(40), [0, 1])])
    await settle()
    expect(calls.length).toBeLessThanOrEqual(2)
    expect(port.requests.filter(request => request.type === "tokens").at(-1)).toMatchObject({ wants: [{ blobId: OBJECT_ID }, { blobId: "b".repeat(40) }] })
  })

  it("leaves every language to the worker without a backend", () => {
    const port = recordingPort()
    const client = new HighlightClient([port])
    client.want("v", [want(OBJECT_ID, [0])])
    expect(port.requests.filter(request => request.type === "text").map(request => request.blobId)).toEqual([OBJECT_ID])
  })
})
