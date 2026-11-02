// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { splitCodeAtBreaks } from "@/lib/inline-code"

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
