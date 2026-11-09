// @spec openspec/specs/commit/commit-row/spec.md
import { describe, expect, it } from "vite-plus/test"
import { codeBreakParts, glueAfter, glueBefore, splitCodeAtBreaks } from "@/lib/inline-code"

describe("splitCodeAtBreaks", () => {
  it("breaks a path after each slash", () => {
    expect(splitCodeAtBreaks("src/lib/commit.ts")).toEqual(["src/", "lib/", "commit.ts"])
  })

  it("keeps the path prefix and each directory slash with the directory", () => {
    expect(splitCodeAtBreaks("./community/tools/bt.cmd")).toEqual(["./", "community/", "tools/", "bt.cmd"])
  })

  it("does not break before a short final extension", () => {
    expect(splitCodeAtBreaks("Foo.kt")).toEqual(["Foo.kt"])
  })

  it("breaks before a dotted name but keeps a chain of short final extensions", () => {
    expect(splitCodeAtBreaks("Foo.generated.kt")).toEqual(["Foo", ".generated.kt"])
    expect(splitCodeAtBreaks("spec/tools/host.spec.md")).toEqual(["spec/", "tools/", "host.spec.md"])
    expect(splitCodeAtBreaks("archive.tar.gz")).toEqual(["archive.tar.gz"])
  })

  it("keeps a leading ./ prefix together", () => {
    expect(splitCodeAtBreaks("./bazel.cmd")).toEqual(["./", "bazel.cmd"])
  })

  it("does not break inside // or ...", () => {
    expect(splitCodeAtBreaks("//plugins/air/...")).toEqual(["//", "plugins/", "air/", "..."])
  })

  it("keeps a CONSTANT_CASE name whole", () => {
    expect(splitCodeAtBreaks("POOL_RETRY_LIMIT")).toEqual(["POOL_RETRY_LIMIT"])
  })
})

describe("the punctuation glued to code", () => {
  it("keeps the closing marks and the end of a sentence after code on its line, never the words after them", () => {
    expect(glueAfter(", the output model")).toBe(",")
    expect(glueAfter("), the model state")).toBe("),")
    expect(glueAfter(". Each new field")).toBe(".")
    expect(glueAfter(" and the rest")).toBe("")
  })

  it("keeps an opening mark before code on its line, never the words before it", () => {
    expect(glueBefore("the output model (")).toBe("(")
    expect(glueBefore("travels in ")).toBe("")
  })
})

describe("codeBreakParts", () => {
  it("breaks a part too long for a line at its humps, a shorter one only at its dots", () => {
    expect(codeBreakParts("TerminalContentUpdatedEvent.rightEdgeLineEnds")).toEqual(["Terminal", "Content", "Updated", "Event", ".rightEdgeLineEnds"])
    expect(codeBreakParts("TerminalOutputModel.getRightEdgeLineEnds")).toEqual(["TerminalOutputModel", ".getRightEdgeLineEnds"])
    expect(codeBreakParts("BOX_DRAWING_CODE_POINTS_OF_THE_TERMINAL")).toEqual(["BOX_DRAWING_CODE_POINTS_OF_THE_TERMINAL"])
  })
})
