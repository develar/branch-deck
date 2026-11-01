// @spec spec/commit/commit-review.spec.md
import { describe, expect, it } from "vite-plus/test"
import { presentCardEnd } from "@/presenters/review-card-end"

describe("presentCardEnd", () => {
  it("names the next file as its header does, its name and its directory, and the next part of the same file as continued", () => {
    expect(presentCardEnd("README.md", ".claude/skills/setup-local-rd-flow/SKILL.md", null)).toEqual({
      kind: "next",
      name: "SKILL.md",
      continued: false,
      label: "Next: SKILL.md .claude/skills/setup-local-rd-flow",
    })
    expect(presentCardEnd("src/a/Manager.kt", "README.md", null)).toEqual({ kind: "next", name: "README.md", continued: false, label: "Next: README.md" })
    expect(presentCardEnd("src/a/Manager.kt", "src/a/Manager.kt", null)).toEqual({ kind: "next", name: "Manager.kt", continued: true, label: "Next: Manager.kt, continued" })
  })

  it("ends the review at the last card, and the files of an owner chip before it", () => {
    expect(presentCardEnd("README.md", undefined, null)).toEqual({ kind: "end", text: "End of the review" })
    expect(presentCardEnd("ui/login.css", undefined, "Design")).toEqual({ kind: "end", text: "No more files of Design" })
  })
})
