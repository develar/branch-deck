// The eager JS of the main window (B13 of docs/design/rust-domain-authority.md; docs/design/rendering-performance.md,
// "Loading"): the entry and the modulepreloads of `index.html`, which every window loads and compiles before its page,
// their bytes and gzip, and from their source maps the bytes of each package and of each module of the page.
//
//   pnpm perf:eager-js                  builds the page into tests/results/perf/eager-js/ and reports that build
//   pnpm perf:eager-js --dist <dir>     reports a build of `vp build --sourcemap hidden` (without maps: the files only)
//   pnpm perf:eager-js --top 60         the 60 biggest modules of the page (40 by default)
//
// The build of the script is `vp build --sourcemap hidden`: the maps without the comment that names them in each file,
// so the bytes are those of `pnpm build`. The total is printed beside the budget of B13, the eager JS of the tree of s0a
// (nothing fails on it), and the report goes to tests/results/perf/eager-js.json too.
import { execFileSync } from "node:child_process"
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import { parseArgs } from "node:util"
import { gzipSync } from "node:zlib"

const root = path.join(import.meta.dirname, "..", "..")
const results = path.join(root, "tests", "results", "perf")

/** B13: the eager JS of the main window in the tree of s0a (`66938306`), which the page must not exceed, bytes */
export const BUDGET = 719_284

/** The bytes a span of generated code has that no source map segment names, and a file without a map */
const UNMAPPED = "(unmapped)"

/** The scripts `index.html` loads before the page: its module entry first, then its modulepreloads, as paths in `dist` */
export function eagerScripts(html) {
  const entries = []
  const preloads = []
  for (const tag of html.matchAll(/<(script|link)\b([^>]*)>/g)) {
    const attributes = Object.fromEntries([...tag[2].matchAll(/([\w-]+)="([^"]*)"/g)].map(match => [match[1], match[2]]))
    if (tag[1] === "script" && attributes.type === "module" && attributes.src !== undefined) {
      entries.push(attributes.src.replace(/^\//, ""))
    } else if (tag[1] === "link" && attributes.rel === "modulepreload" && attributes.href !== undefined) {
      preloads.push(attributes.href.replace(/^\//, ""))
    }
  }
  return { entries, preloads }
}

const BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"

/** The fields of one segment of the `mappings` of a source map (Base64 VLQ), each a delta */
export function decodeSegment(segment) {
  const fields = []
  let value = 0
  let shift = 0
  for (const char of segment) {
    const digit = BASE64.indexOf(char)
    value += (digit & 31) << shift
    if (digit & 32) {
      shift += 5
    } else {
      fields.push(value & 1 ? -(value >>> 1) : value >>> 1)
      value = 0
      shift = 0
    }
  }
  return fields
}

/**
 * The UTF-8 bytes of `code` by the source its map names for them (`sources` as written in the map); a span no segment
 * names, a newline included, is `UNMAPPED`. Columns of a map count UTF-16 units, so each span is measured as its text
 */
export function bytesBySource(code, map) {
  const bytes = new Map()
  const add = (source, count) => bytes.set(source, (bytes.get(source) ?? 0) + count)
  const lines = code.split("\n")
  const mappings = map.mappings.split(";")
  // the index of the source is a delta across the whole map, the generated column one within its line
  let source = 0
  lines.forEach((line, index) => {
    if (index < lines.length - 1) {
      add(UNMAPPED, 1)
    }
    const spans = []
    let column = 0
    for (const segment of (mappings[index] ?? "").split(",").filter(it => it !== "")) {
      const fields = decodeSegment(segment)
      column += fields[0]
      if (fields.length >= 4) {
        source += fields[1]
      }
      spans.push({ column, source: fields.length >= 4 ? (map.sources[source] ?? UNMAPPED) : UNMAPPED })
    }
    add(UNMAPPED, Buffer.byteLength(line.slice(0, spans[0]?.column ?? line.length)))
    spans.forEach((span, spanIndex) => {
      add(span.source, Buffer.byteLength(line.slice(span.column, spans[spanIndex + 1]?.column ?? line.length)))
    })
  })
  return bytes
}

/**
 * What a source of a map in `directory` is: a package by its directory under `node_modules`, a module of the page by its
 * path in the repository, or the bundler's own code (its runtime, a virtual module)
 */
export function ownerOf(source, directory) {
  if (source.includes("\0") || /^[a-z]+:/.test(source)) {
    return { kind: "other", name: source.replaceAll("\0", "") }
  }
  const file = path.relative(root, path.resolve(directory, source)).replaceAll("\\", "/")
  const packageName = /(?:^|\/)node_modules\/(?:.*\/node_modules\/)?((?:@[^/]+\/)?[^/]+)\//.exec(file)?.[1]
  if (packageName !== undefined) {
    return { kind: "package", name: packageName }
  }
  return file.startsWith("src/") ? { kind: "module", name: file } : { kind: "other", name: file }
}

/** The eager scripts of the build in `dist`: their bytes and gzip, and the bytes of each owner of their sources */
export function measure(dist) {
  const { entries, preloads } = eagerScripts(readFileSync(path.join(dist, "index.html"), "utf8"))
  const files = []
  const owners = { package: new Map(), module: new Map(), other: new Map() }
  let mapped = 0
  for (const file of [...entries, ...preloads]) {
    const code = readFileSync(path.join(dist, file))
    files.push({ file, entry: entries.includes(file), bytes: code.length, gzip: gzipSync(code).length })
    const mapFile = path.join(dist, `${file}.map`)
    if (!existsSync(mapFile)) {
      owners.other.set(UNMAPPED, (owners.other.get(UNMAPPED) ?? 0) + code.length)
      continue
    }
    mapped++
    const map = JSON.parse(readFileSync(mapFile, "utf8"))
    const directory = path.resolve(path.dirname(mapFile), map.sourceRoot ?? "")
    const bySource = bytesBySource(code.toString("utf8"), map)
    const counted = [...bySource.values()].reduce((sum, bytes) => sum + bytes, 0)
    if (counted !== code.length) {
      throw new Error(`${file}: the spans of its map count ${counted} bytes of ${code.length}`)
    }
    for (const [source, bytes] of bySource) {
      const owner = source === UNMAPPED ? { kind: "other", name: UNMAPPED } : ownerOf(source, directory)
      owners[owner.kind].set(owner.name, (owners[owner.kind].get(owner.name) ?? 0) + bytes)
    }
  }
  const sum = key => files.reduce((total, file) => total + file[key], 0)
  const sorted = map => Object.fromEntries([...map].sort((a, b) => b[1] - a[1]))
  return {
    files,
    total: { bytes: sum("bytes"), gzip: sum("gzip") },
    budget: BUDGET,
    mappedFiles: mapped,
    packages: sorted(owners.package),
    modules: sorted(owners.module),
    other: sorted(owners.other),
  }
}

const number = value => value.toLocaleString("en-US")
const row = (...cells) => cells.map((cell, index) => (index < cells.length - 1 ? String(cell).padStart(10) : `  ${cell}`)).join("")

/** The report of `measure` as text */
export function format(report, { dist, top }) {
  const lines = []
  const preloads = report.files.filter(file => !file.entry).length
  lines.push(`The eager JS of ${path.relative(root, dist) || dist}/index.html: the entry and ${preloads} modulepreloads`, row("bytes", "gzip", "file"))
  for (const file of report.files) {
    lines.push(row(number(file.bytes), number(file.gzip), file.entry ? `${file.file} (entry)` : file.file))
  }
  const over = report.total.bytes - report.budget
  const verdict = over > 0 ? `${number(over)} over` : `${number(-over)} under`
  lines.push(row(number(report.total.bytes), number(report.total.gzip), `total; the budget of B13 ${number(report.budget)} bytes (the tree of s0a): ${verdict}`))
  if (report.mappedFiles === 0) {
    lines.push("", "No source maps: the build of `vp build --sourcemap hidden` says what the bytes are")
    return lines.join("\n")
  }
  const total = owners => Object.values(owners).reduce((sum, bytes) => sum + bytes, 0)
  const section = (title, owners, limit) => {
    const entries = Object.entries(owners)
    lines.push("", `${title}: ${number(total(owners))} bytes${entries.length > limit ? `, the ${limit} biggest of ${entries.length}` : ""}`)
    for (const [name, bytes] of entries.slice(0, limit)) {
      lines.push(row(number(bytes), name))
    }
  }
  section("Packages", report.packages, Number.POSITIVE_INFINITY)
  section("Modules of the page", report.modules, top)
  section("The bundler's code and what no map names", report.other, Number.POSITIVE_INFINITY)
  return lines.join("\n")
}

function main() {
  const { values } = parseArgs({ options: { dist: { type: "string" }, top: { type: "string", default: "40" } } })
  let dist = values.dist === undefined ? undefined : path.resolve(values.dist)
  if (dist === undefined) {
    dist = path.join(results, "eager-js")
    execFileSync("pnpm", ["exec", "vp", "build", "--sourcemap", "hidden", "--outDir", dist, "--logLevel", "warn"], { cwd: root, stdio: "inherit" })
  }
  const report = measure(dist)
  console.log(format(report, { dist, top: Number(values.top) }))
  mkdirSync(results, { recursive: true })
  writeFileSync(path.join(results, "eager-js.json"), `${JSON.stringify(report, null, 2)}\n`)
}

if (import.meta.main) {
  main()
}
