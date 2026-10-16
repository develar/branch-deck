// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { splitCodeAtBreaks, splitInlineCode } from "@/lib/inline-code"
import type { InlineSegment } from "@/lib/inline-code"

function text(content: string): InlineSegment {
  return { type: "text", content, explicit: false }
}

function code(content: string, explicit = false): InlineSegment {
  return { type: "code", content, explicit }
}

describe("splitInlineCode", () => {
  it("shows text in backticks as explicit code without the backticks", () => {
    expect(splitInlineCode("Use `pool size` here")).toEqual([text("Use "), code("pool size", true), text(" here")])
  })

  it.each([
    ["relative path", "./tools/bt.cmd"],
    ["parent path", "../shared/config"],
    ["home path", "~/projects/app"],
    ["Bazel target pattern", "//plugins/air/..."],
    ["path with two slashes", "src/lib/commit.ts"],
    ["file name with dots", "Foo.generated.kt"],
    ["dotted module name", "intellij.air.backend.tests"],
    ["dotted name ending in a member", "com.intellij.air.backend.workflows.sessionFinished"],
    ["long flag", "--lane"],
    ["short flag", "-p"],
    ["flag with a value", "--level=debug"],
    ["call", "run()"],
    ["member call", "Foo.bar()"],
    ["Kotlin member reference", "Foo::bar"],
    ["Javadoc member reference", "Foo#bar"],
    ["CONSTANT_CASE", "POOL_RETRY_LIMIT"],
    ["snake_case", "snake_case"],
    ["camelCase", "afterLock"],
    ["PascalCase with three humps", "SessionFolderStore"],
  ])("detects a %s as code", (_kind, token) => {
    expect(splitInlineCode(`see ${token} now`)).toEqual([text("see "), code(token), text(" now")])
  })

  it.each([
    ["an acronym", "HTTP"],
    ["a short camelCase word", "aB"],
    ["a PascalCase name with two humps", "GenUI"],
    ["a PascalCase name ending in a capital", "AssertJ"],
    ["a host name of two parts", "example.com"],
    ["an abbreviation with dots", "e.g."],
    ["JavaScript from the stoplist", "JavaScript"],
    ["macOS from the stoplist", "macOS"],
  ])("keeps %s as prose", (_kind, token) => {
    expect(splitInlineCode(`see ${token} now`)).toEqual([text(`see ${token} now`)])
  })

  it("keeps a token in double quotes as prose", () => {
    expect(splitInlineCode('rename "pool_size" later')).toEqual([text('rename "pool_size" later')])
  })

  it("keeps the punctuation around a token as prose", () => {
    expect(splitInlineCode("see (db/pool.js), then")).toEqual([text("see ("), code("db/pool.js"), text("), then")])
  })

  it("joins code tokens separated by a single space into one span", () => {
    expect(splitInlineCode("run bt.cmd --lane now")).toEqual([text("run "), code("bt.cmd --lane"), text(" now")])
  })

  it("joins a flag and its dotted value into one span", () => {
    expect(splitInlineCode("bt --module intellij.air.tests --filter com.intellij.air: 5 passed")).toEqual([
      text("bt "),
      code("--module intellij.air.tests --filter com.intellij.air"),
      text(": 5 passed"),
    ])
  })

  it("ends a code span at trailing punctuation", () => {
    expect(splitInlineCode("edit foo.ts, bar.ts")).toEqual([text("edit "), code("foo.ts"), text(", "), code("bar.ts")])
  })

  it("keeps the punctuation after a call as prose", () => {
    expect(splitInlineCode("call (run()).")).toEqual([text("call ("), code("run()"), text(").")])
  })

  it("keeps an issue key inside a code token", () => {
    expect(splitInlineCode("see docs/JIRA-456/retry.md now")).toEqual([text("see "), code("docs/JIRA-456/retry.md"), text(" now")])
  })
})

describe("splitCodeAtBreaks", () => {
  it("breaks a path before each slash", () => {
    expect(splitCodeAtBreaks("src/lib/commit.ts")).toEqual(["src", "/lib", "/commit.ts"])
  })

  it("does not break before a short final extension", () => {
    expect(splitCodeAtBreaks("Foo.kt")).toEqual(["Foo.kt"])
  })

  it("breaks before a dot that is not the final extension", () => {
    expect(splitCodeAtBreaks("Foo.generated.kt")).toEqual(["Foo", ".generated.kt"])
  })

  it("does not break inside a leading ./", () => {
    expect(splitCodeAtBreaks("./bazel.cmd")).toEqual(["./bazel.cmd"])
  })

  it("does not break inside // or ...", () => {
    expect(splitCodeAtBreaks("//plugins/air/...")).toEqual(["//plugins", "/air", "/..."])
  })

  it("keeps a CONSTANT_CASE name whole", () => {
    expect(splitCodeAtBreaks("POOL_RETRY_LIMIT")).toEqual(["POOL_RETRY_LIMIT"])
  })
})
