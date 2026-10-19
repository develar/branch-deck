// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import { HighlightClient, inProcessPort } from "@/lib/diff/highlight/client"
import type { BlobWant, HighlightPort } from "@/lib/diff/highlight/client"
import { createGrammarSource } from "@/lib/diff/highlight/grammars"
import type { HighlightAnswer, HighlightRequest } from "@/lib/diff/highlight/protocol"
import { TokenCache } from "@/lib/diff/highlight/token-cache"

const source = createGrammarSource()

/** A port that records the requests and lets a test answer */
function recordingPort(): HighlightPort & { requests: HighlightRequest[]; answer: (answer: HighlightAnswer) => void } {
  const listeners: ((answer: HighlightAnswer) => void)[] = []
  const requests: HighlightRequest[] = []
  return {
    requests,
    post: request => requests.push(request),
    listen: listener => listeners.push(listener),
    answer: answer => listeners.forEach(listener => listener(answer)),
  }
}

function want(blobId: string, pages: number[], text = "val a = 1"): BlobWant {
  return { blobId, language: "kotlin", text, pages, priority: 0 }
}

const page = { lineStarts: new Uint32Array([0, 0]), runs: new Uint32Array() }

describe("HighlightClient", () => {
  it("pins each blob to the least loaded port and sends its text once", () => {
    const ports = [recordingPort(), recordingPort()]
    const client = new HighlightClient(ports)
    client.want("v", [want("a", [0], "x".repeat(100)), want("b", [0])])
    client.want("v", [want("a", [0, 1], "x".repeat(100)), want("b", [0])])
    const texts = ports.map(port => port.requests.filter(request => request.type === "text").map(request => request.blobId))
    expect(texts).toEqual([["a"], ["b"]])
    // each port gets the list of the view, an empty one too, so it drops the work asked for before
    expect(ports[1]!.requests.filter(request => request.type === "tokens").at(-1)).toEqual({ type: "tokens", view: "v", wants: [{ blobId: "b", pages: [0], priority: 0 }] })
  })

  it("asks only for the pages the cache lacks exact, and never for plain text", () => {
    const port = recordingPort()
    const cache = new TokenCache()
    const client = new HighlightClient([port], cache)
    cache.put("a", 0, { exact: true, ...page })
    cache.put("a", 1, { exact: false, ...page })
    client.want("v", [want("a", [0, 1, 2]), { ...want("t", [0]), language: "text" }])
    expect(port.requests.at(-1)).toEqual({ type: "tokens", view: "v", wants: [{ blobId: "a", pages: [1, 2], priority: 0 }] })
  })

  it("sends the text again when the worker dropped it, and stops asking for a blob that failed", () => {
    const port = recordingPort()
    const failures: string[] = []
    const client = new HighlightClient([port], new TokenCache(), blobId => failures.push(blobId))
    client.want("v", [want("a", [0]), want("f", [0])])
    port.answer({ type: "missing", blobId: "a" })
    expect(port.requests.filter(request => request.type === "text" && request.blobId === "a")).toHaveLength(2)
    port.answer({ type: "failed", blobId: "f", reason: "no grammar" })
    client.want("v", [want("f", [0])])
    expect([failures, port.requests.at(-1)]).toEqual([["f"], { type: "tokens", view: "v", wants: [] }])
  })

  it("keeps the pages the engine on the main thread answers, a provisional one confirmed exact", async () => {
    const port = inProcessPort(source)
    const cache = new TokenCache()
    const arrived: string[] = []
    cache.subscribe((blobId, index) => arrived.push(`${blobId}:${index}`))
    const client = new HighlightClient([port], cache)
    client.want("v", [want("a", [0], "class A")])
    await port.idle()
    expect([arrived, cache.get("a", 0)?.exact]).toEqual([["a:0"], true])
  })
})

describe("TokenCache", () => {
  it("drops the least recently used blob over its budget, never the one just put", () => {
    const big = { exact: true, lineStarts: new Uint32Array(2), runs: new Uint32Array(30) }
    const cache = new TokenCache(256)
    cache.put("a", 0, big)
    cache.put("b", 0, big)
    cache.touch("a")
    cache.put("c", 0, big)
    expect(["a", "b", "c"].map(blobId => cache.get(blobId, 0) !== undefined)).toEqual([true, false, true])
    cache.put("d", 0, { ...big, runs: new Uint32Array(100) })
    expect(cache.get("d", 0)).toBeDefined()
  })
})
