/** Plain prose or a code token (identifier, path, command, flag) inside a commit message line. */
export interface InlineSegment {
  type: "text" | "code"
  content: string
  /** Written in backticks by the author, as opposed to detected */
  explicit: boolean
}

const BACKTICK_SPAN = /`([^`]+)`/g
// a token is a run of non-whitespace; the punctuation around it stays prose
const TOKEN = /\S+/g
const LEADING_PUNCTUATION = /^[("'[]+/
const TRAILING_PUNCTUATION = /[)\]"',.;:!?]+$/

const CODE_EXTENSIONS = "kt|kts|java|ts|tsx|js|mjs|cjs|rs|go|py|json|toml|ya?ml|md|cmd|sh|bazel|bzl|gradle|xml|css|html|iml|properties"
const CODE_PATTERNS: readonly RegExp[] = [
  // paths: ./tools/bt.cmd, ../x, ~/x, //plugins/air/...
  /^(\.{1,2}\/|~\/|\/\/)\S+$/,
  // relative paths with two or more slashes: src/lib/commit.ts, a/b/c
  /^\w[\w.-]*(\/[\w.-]+){2,}\/?$/,
  // file names: Foo.generated.kt, db/pool.js
  new RegExp(`^[\\w-]+(\\/[\\w.-]+)*(\\.[\\w-]+)*\\.(${CODE_EXTENSIONS})$`),
  // dotted names of modules and packages, from three parts: intellij.air.backend.tests, com.intellij.Foo
  // (two parts are too often prose: example.com, e.g.)
  /^[a-z][\w-]*(\.[A-Za-z_][\w-]*){2,}$/,
  // flags: --lane, -p, --level=debug
  /^--?[a-z][\w-]*(=\S+)?$/,
  // calls and members: run(), Foo.bar(), Foo::bar, Foo#bar
  /^\w+(\.\w+)*\(\)$/,
  /^\w+::\w+$/,
  /^[A-Za-z]\w*#\w+$/,
  // CONSTANT_CASE needs an underscore, so acronyms (HTTP, API) stay prose
  /^[A-Z][A-Z0-9]*(_[A-Z0-9]+)+$/,
  /^[a-z][a-z0-9]*(_[a-z0-9]+)+$/,
  // camelCase from five characters: afterLock, whileDownReads
  /^(?=.{5})[a-z][a-z0-9]*[A-Z][A-Za-z0-9]*$/,
  // PascalCase with three or more humps: SessionFolderStore (two-hump words and acronyms such as GenUI are too often names)
  /^[A-Z][a-z0-9]+([A-Z][a-z0-9]+){2,}$/,
]

// camel-cased names of products and companies that the patterns would take for identifiers
const STOP_WORDS: ReadonlySet<string> = new Set([
  "macOS",
  "iOS",
  "iPadOS",
  "watchOS",
  "tvOS",
  "visionOS",
  "iPhone",
  "iPad",
  "iCloud",
  "iMac",
  "eBay",
  "JavaScript",
  "TypeScript",
  "GitHub",
  "GitLab",
  "IntelliJ",
  "JetBrains",
  "PyCharm",
  "WebStorm",
  "YouTube",
  "PowerPoint",
  "PowerShell",
  "LinkedIn",
  "OAuth",
  "GraphQL",
  "PostgreSQL",
  "MySQL",
  "MongoDB",
  "WebKit",
  "WebView",
  "DevTools",
  "OpenType",
  "PayPal",
  "FedEx",
  "MacBook",
  "NuGet",
])

/**
 * Splits text into prose and code: spans in backticks, and tokens that look like code (paths, file names, dotted module
 * and package names, flags, calls, CONSTANT_CASE, snake_case, camelCase, PascalCase with three or more humps), except
 * the names in a stoplist
 * and tokens in double quotes. Consecutive code tokens separated by single spaces form one span (`bt.cmd --lane`).
 */
export function splitInlineCode(text: string): InlineSegment[] {
  const segments: InlineSegment[] = []
  let lastEnd = 0
  for (const match of text.matchAll(BACKTICK_SPAN)) {
    detectCode(text.slice(lastEnd, match.index), segments)
    segments.push({ type: "code", content: match[1]!, explicit: true })
    lastEnd = match.index + match[0].length
  }
  detectCode(text.slice(lastEnd), segments)
  return segments
}

function detectCode(text: string, segments: InlineSegment[]) {
  let proseStart = 0
  let codeStart = -1
  let codeEnd = -1
  const flushCode = () => {
    if (codeStart !== -1) {
      pushText(text.slice(proseStart, codeStart), segments)
      segments.push({ type: "code", content: text.slice(codeStart, codeEnd), explicit: false })
      proseStart = codeEnd
      codeStart = -1
    }
  }

  for (const match of text.matchAll(TOKEN)) {
    const token = match[0]
    const leading = LEADING_PUNCTUATION.exec(token)?.[0].length ?? 0
    let core = token.slice(leading).replace(TRAILING_PUNCTUATION, "")
    // Bazel's recursive target pattern ends with dots: //plugins/air/...
    if (core.endsWith("/") && token.startsWith("...", leading + core.length)) {
      core += "..."
    }
    // the closing parenthesis of a call is part of it: run(), Foo.bar()
    else if (core.endsWith("(") && token[leading + core.length] === ")") {
      core += ")"
    }
    const quoted = token.startsWith('"')
    const start = match.index + leading
    const adjacent = codeStart !== -1 && start === codeEnd + 1 && leading === 0
    if (core.length === 0 || quoted || !isCode(core)) {
      flushCode()
      continue
    }

    // continue the span over a single space (`bt.cmd --lane`), not across punctuation or a line of prose
    if (!adjacent) {
      flushCode()
      codeStart = start
    }
    codeEnd = start + core.length
    // trailing punctuation ends the span
    if (core.length + leading < token.length) {
      flushCode()
    }
  }
  flushCode()
  pushText(text.slice(proseStart), segments)
}

function isCode(token: string): boolean {
  return !STOP_WORDS.has(token) && CODE_PATTERNS.some(pattern => pattern.test(token))
}

function pushText(content: string, segments: InlineSegment[]) {
  if (content.length === 0) {
    return
  }
  const last = segments.at(-1)
  if (last?.type === "text") {
    last.content += content
  } else {
    segments.push({ type: "text", content, explicit: false })
  }
}

// break opportunities in long paths and dotted names: before a `/` or `.` that follows a name character (not inside
// `//`, `./` or Bazel's `...`), and not before a short final extension (`Foo` / `.kt`); CONSTANT_CASE names stay whole
const CODE_BREAK = /(?<=[^/.])(?=\/)|(?<=[^.])(?=\.(?![./]|\w{1,4}$))/

/** Splits a code token at its line-break opportunities: `src/lib/commit.ts` → `src/`, `lib/`, `commit.ts`. */
export function splitCodeAtBreaks(content: string): string[] {
  return content.split(CODE_BREAK)
}
