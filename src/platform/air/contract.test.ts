// The copy of the contract of Air (openspec/specs/repository/ide-embedding/spec.md, "Contract: The copy of the contract
// is valid against its schema"): each line of docs/design/ide-embedding/protocol/corpus/*.ndjson against
// x-air-app.schema.json, each result and action the line schema takes as any value against the def of its method (the
// README of the copy), and each app file, the example and those the corpus writes, against air-app.schema.json.
// Ajv validates JSON Schema 2020-12, the draft of both schemas
import { Ajv2020 } from "ajv/dist/2020.js"
import type { ErrorObject, ValidateFunction } from "ajv/dist/2020.js"
import { describe, expect, it } from "vite-plus/test"
import { parse } from "yaml"

const PROTOCOL = "/docs/design/ide-embedding/protocol"
const raw = (files: Record<string, string>) => Object.fromEntries(Object.entries(files).map(([path, text]) => [path.slice(PROTOCOL.length + 1), text]))
const FILES = raw(
  import.meta.glob<string>(
    ["/docs/design/ide-embedding/protocol/*.json", "/docs/design/ide-embedding/protocol/corpus/*.ndjson", "/docs/design/ide-embedding/protocol/examples/*.yaml"],
    { query: "?raw", import: "default", eager: true },
  ),
)

const lineSchema = JSON.parse(FILES["x-air-app.schema.json"]!) as { $id: string }
const appSchema = JSON.parse(FILES["air-app.schema.json"]!) as object
const ajv = new Ajv2020({ strict: false, allErrors: true })
const validLine = ajv.compile(lineSchema)
const validApp = ajv.compile(appSchema)

/** The def `name` of the line schema */
function def(name: string): ValidateFunction {
  const validate = ajv.getSchema(`${lineSchema.$id}#/$defs/${name}`)
  if (validate === undefined) {
    throw new Error(`x-air-app.schema.json has no def ${name}`)
  }
  return validate
}

/** The def of the result of each request, by its link and its method (the README of the copy, "A line") */
const RESULTS: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  ide: { "x-air-ide/appViewTicket": "AppViewTicketResult" },
  page: { initialize: "PageInitializeResult", "x-air-app/viewOpen": "ViewOpenResult" },
  app: { initialize: "AppInitializeResult", "x-air-app/subscribe": "AppSubscribeResult", "x-air-app/openView": "AppOpenViewResult" },
  frame: { "ui/initialize": "FrameInitializeResult", "x-air-app/subscribe": "FrameSubscribeResult" },
}

/** The value at the JSON pointer `pointer` of `data` */
function at(data: unknown, pointer: string): unknown {
  return pointer
    .split("/")
    .slice(1)
    .reduce<unknown>(
      (value, key) => (value === null || typeof value !== "object" ? undefined : (value as Record<string, unknown>)[key.replaceAll("~1", "/").replaceAll("~0", "~")]),
      data,
    )
}

/**
 * The errors of `validate` on `data` that are no placeholder: `${any}` and `${name}` of a line stand for any value, so
 * an error at one, and the errors of a branch that did not apply (`oneOf`, `if`, `anyOf`), are none
 */
function problems(validate: ValidateFunction, data: unknown): string[] {
  if (validate(data)) {
    return []
  }
  const placeholder = (error: ErrorObject) => {
    const value = at(data, error.instancePath)
    return typeof value === "string" && /^\$\{[^}]+\}$/.test(value)
  }
  return (validate.errors ?? [])
    .filter(error => !placeholder(error) && !["oneOf", "if", "anyOf"].includes(error.keyword))
    .map(error => `${error.instancePath} ${error.message} ${JSON.stringify(error.params)}`)
}

interface Line {
  readonly link?: string
  readonly from?: string
  readonly message?: {
    readonly id?: unknown
    readonly method?: string
    readonly result?: unknown
    readonly params?: { readonly action?: { readonly type?: string; readonly key?: string; readonly value?: { readonly problem?: { readonly reason?: string } } } }
  }
  readonly step?: string
  readonly path?: string
  readonly text?: string
  readonly state?: { readonly link: string; readonly state: unknown }
}

/** The reasons of a problem of an app file that a schema sees: the file does not parse, or its format is another */
const INVALID_FILE = new Set(["invalidFile", "unsupportedFormat"])

/**
 * The reason of the problem the first entry of the app `id` after the line `index` names, before the next write of its
 * file; null without one
 */
function problemOfEntry(lines: readonly Line[], index: number, id: string): string | null {
  const path = lines[index]?.path
  for (const line of lines.slice(index + 1)) {
    if (line.step !== undefined && line.path === path) {
      return null
    }
    const action = line.message?.params?.action
    if (line.link === "ide" && line.message?.method === "action" && action?.type === "x-air-ide/appEntrySet" && action.key === id) {
      return action.value?.problem?.reason ?? null
    }
  }
  return null
}

/** The problems of the corpus file `name`, one line each */
function corpusProblems(name: string, text: string): string[] {
  const found: string[] = []
  if (!text.endsWith("\n")) {
    found.push(`${name}: no newline at its end`)
  }
  const texts = text.split("\n").filter(line => line !== "")
  const lines = texts.map((line, index) => {
    try {
      return JSON.parse(line) as Line
    } catch {
      found.push(`${name}:${index + 1} is no JSON`)
      return {}
    }
  })
  const requests = new Map<string, string>()
  lines.forEach((line, index) => {
    const where = `${name}:${index + 1}`
    if (JSON.stringify(line) !== texts[index]) {
      found.push(`${where} is not compact JSON`)
    }
    found.push(...problems(validLine, line).map(problem => `${where} ${problem}`))
    const message = line.message
    if (message !== undefined && line.link !== undefined) {
      if (message.method !== undefined && message.id !== undefined) {
        requests.set(`${line.link}:${JSON.stringify(message.id)}`, message.method)
      }
      const method = message.result === undefined ? undefined : requests.get(`${line.link}:${JSON.stringify(message.id)}`)
      const result = method === undefined ? undefined : RESULTS[line.link]?.[method]
      if (result !== undefined) {
        found.push(...problems(def(result), message.result).map(problem => `${where} the result of ${method} (${result}): ${problem}`))
      }
      const action = message.params?.action
      if (line.link === "ide" && message.method === "action" && action !== undefined) {
        const shape = action.type === "x-air-ide/appEntrySet" || action.type === "x-air-ide/appEntryRemoved" ? "AppsAction" : "AppsIntent"
        found.push(...problems(def(shape), action).map(problem => `${where} the action (${shape}): ${problem}`))
      }
      const snapshot = (message.result as { snapshot?: { state?: { apps?: unknown } } } | undefined)?.snapshot
      if (line.link === "ide" && snapshot?.state?.apps !== undefined) {
        found.push(...problems(def("AppsState"), snapshot.state).map(problem => `${where} the snapshot (AppsState): ${problem}`))
      }
    }
    if (line.state?.link === "ide") {
      found.push(...problems(def("AppsState"), line.state.state).map(problem => `${where} the state (AppsState): ${problem}`))
    }
    if (line.step === "file" && line.path?.endsWith(".yaml") === true && line.text !== undefined) {
      const id = line.path.slice(line.path.lastIndexOf("/") + 1, -".yaml".length)
      const expectedInvalid = INVALID_FILE.has(problemOfEntry(lines, index, id) ?? "")
      let file: unknown
      try {
        file = parse(line.text) as unknown
      } catch {
        if (!expectedInvalid) {
          found.push(`${where} the app file ${id}.yaml does not parse`)
        }
        return
      }
      const fileProblems = problems(validApp, file)
      if (expectedInvalid && fileProblems.length === 0) {
        found.push(`${where} the app file ${id}.yaml is valid, while its entry names a problem of the file`)
      } else if (!expectedInvalid) {
        found.push(...fileProblems.map(problem => `${where} the app file ${id}.yaml: ${problem}`))
      }
    }
  })
  return found
}

describe("The copy of the contract", () => {
  it("each line of the corpus is valid, each result and action by the def of its method, and each app file the corpus writes", () => {
    const corpus = Object.entries(FILES).filter(([path]) => path.startsWith("corpus/"))
    expect(corpus.map(([path]) => path)).toContain("corpus/branch-deck-app.ndjson")
    expect(corpus.flatMap(([path, text]) => corpusProblems(path, text))).toEqual([])
  })

  it("the example app file is valid against air-app.schema.json", () => {
    const example = parse(FILES["examples/branch-deck.yaml"]!) as unknown
    expect(problems(validApp, example)).toEqual([])
    // the schema has teeth: a file of another major is refused
    expect(validApp({ ...(example as object), format: 2 })).toBe(false)
  })
})
