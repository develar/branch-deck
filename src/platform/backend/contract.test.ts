// The contract of the one backend (openspec/specs/repository/backend-protocol/spec.md, "Wire: Every message is one
// JSON-RPC 2.0 object on one line"): each frame of docs/design/backend-protocol/corpus/*.ndjson against
// backend.schema.json, each result also against the def of the method of its request. A token `${name}` of a line stands
// for any value, so an error at one, and the errors of a branch that did not apply (`oneOf`, `if`, `anyOf`), are none; a
// client line marked invalid breaks the schema on purpose. The host runs the same corpus (crates/backend-host/src/corpus_test.rs)
import { Ajv2020 } from "ajv/dist/2020.js"
import type { ValidateFunction } from "ajv/dist/2020.js"
import { describe, expect, it } from "vite-plus/test"
import { schemaProblems } from "@/test-support/schema-problems"

const PROTOCOL = "/docs/design/backend-protocol"
const FILES = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>(["/docs/design/backend-protocol/*.json", "/docs/design/backend-protocol/corpus/*.ndjson"], { query: "?raw", import: "default", eager: true }),
  ).map(([path, text]) => [path.slice(PROTOCOL.length + 1), text]),
)

const schema = JSON.parse(FILES["backend.schema.json"]!) as { $id: string }
const ajv = new Ajv2020({ strict: false, allErrors: true })
const validFrame = ajv.compile(schema)

/** The def of the shape of `frame`, as the `oneOf` of `Message` takes it: a request, a notification, a result, an error */
function shapeOf(frame: Frame): ValidateFunction {
  if (frame.method !== undefined) {
    return def(frame.id === undefined ? "Notification" : "Request")
  }
  return def(frame.result === undefined ? "Error" : "Result")
}

/** The def `name` of the schema */
function def(name: string): ValidateFunction {
  const validate = ajv.getSchema(`${schema.$id}#/$defs/${name}`)
  if (validate === undefined) {
    throw new Error(`backend.schema.json has no def ${name}`)
  }
  return validate
}

/** The def of the result of each method; a command of the registry answers `CommandResult` */
const RESULTS: Readonly<Record<string, string>> = {
  initialize: "InitializeResult",
  ping: "EmptyResult",
  subscribe: "SubscribeResult",
  unsubscribe: "EmptyResult",
  "x-branch-deck/repositoryChannel": "RepositoryChannelResult",
  "x-branch-deck/setDemand": "EmptyResult",
  "x-branch-deck/followPrefix": "EmptyResult",
  "x-branch-deck/claimNews": "ClaimNewsResult",
  "x-branch-deck/showMain": "EmptyResult",
  "x-branch-deck/openWindow": "OpenWindowResult",
  "x-branch-deck/showWindow": "EmptyResult",
}

/** The def of the result of `method` */
function resultOf(method: string): string | undefined {
  return RESULTS[method] ?? (/^x-branch-deck\/[a-z][a-z0-9_]*$/.test(method) ? "CommandResult" : undefined)
}

interface Frame {
  readonly id?: unknown
  readonly method?: string
  readonly result?: unknown
}

interface Line {
  readonly comment?: string
  readonly client?: Frame
  readonly clientRaw?: string
  readonly host?: Frame
  readonly step?: string
  readonly conn?: number
  readonly invalid?: boolean
}

/** The problems of the corpus file `name`, one line each */
function corpusProblems(name: string, text: string): string[] {
  const found: string[] = []
  if (!text.endsWith("\n")) {
    found.push(`${name}: no newline at its end`)
  }
  // the method of each request by its side, its connection and its id: a result answers the request of the other side
  const requests = new Map<string, string>()
  text
    .split("\n")
    .filter(line => line !== "")
    .forEach((text, index) => {
      const where = `${name}:${index + 1}`
      let line: Line
      try {
        line = JSON.parse(text) as Line
      } catch {
        found.push(`${where} is no JSON`)
        return
      }
      if (JSON.stringify(line) !== text) {
        found.push(`${where} is not compact JSON`)
      }
      const conn = line.conn ?? 1
      for (const [side, other, frame] of [
        ["client", "host", line.client],
        ["host", "client", line.host],
      ] as const) {
        if (frame === undefined) {
          continue
        }
        if (!(side === "client" && line.invalid === true)) {
          found.push(...schemaProblems(shapeOf(frame), frame).map(problem => `${where} ${problem}`))
        }
        if (frame.method !== undefined && frame.id !== undefined) {
          requests.set(`${side}:${conn}:${JSON.stringify(frame.id)}`, frame.method)
        }
        const method = frame.result === undefined ? undefined : requests.get(`${other}:${conn}:${JSON.stringify(frame.id)}`)
        const result = method === undefined ? undefined : resultOf(method)
        if (result !== undefined) {
          found.push(...schemaProblems(def(result), frame.result).map(problem => `${where} the result of ${method} (${result}): ${problem}`))
        }
      }
    })
  return found
}

describe("The contract of the backend", () => {
  it("each frame of the corpus is valid, each result by the def of its method", () => {
    const corpus = Object.entries(FILES).filter(([path]) => path.startsWith("corpus/"))
    expect(corpus.map(([path]) => path).toSorted()).toEqual([
      "corpus/command-stream.ndjson",
      "corpus/connection.ndjson",
      "corpus/errors.ndjson",
      "corpus/host-requests.ndjson",
      "corpus/repository-channel.ndjson",
    ])
    expect(corpus.flatMap(([path, text]) => corpusProblems(path, text))).toEqual([])
  })

  it("the schema has teeth: a frame without its channel and a result of another shape are refused", () => {
    expect(validFrame({ jsonrpc: "2.0", id: 1, method: "ping", params: {} })).toBe(false)
    expect(schemaProblems(def("SubscribeResult"), { snapshot: { fromSeq: 0 } })).not.toEqual([])
  })
})
