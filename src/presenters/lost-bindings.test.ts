// @spec openspec/specs/repository/update-project/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { FirstConflict, UpdateCommit, UpdateLostBinding } from "@/lib/bindings"
import { describeLostBinding, lostBindingsOf } from "@/presenters/lost-bindings"

function commit(id: string, subject: string, files: string[]): UpdateCommit {
  return { id: id.repeat(40), subject, author: "Dana Lee", time: 1_700_000_000, committer: "Dana Lee", committerTime: 1_700_000_000, files }
}

const backend: UpdateLostBinding = { file: "src/Launch.kt", name: "backend", binding: "declaration", useLine: 11, bindingLine: 8 }
const rethrow: UpdateLostBinding = { file: "src/Launch.kt", name: "rethrow", binding: "import", useLine: 12, bindingLine: 4 }

function conflict(upstreamCommits: UpdateCommit[]): FirstConflict {
  return {
    commit: commit("b", "(launch) Drop the old path", ["src/Launch.kt"]),
    files: ["src/Launch.kt"],
    fileCount: 1,
    upstreamCommits,
    lostBindings: [backend, rethrow],
  }
}

describe("describeLostBinding", () => {
  it("names the commit, what it removes, and the upstream commit of the file that uses it", () => {
    const first = conflict([commit("c", "Start with the backend", ["src/Launch.kt"]), commit("d", "Add the guide", ["docs/guide.md"])])
    expect(describeLostBinding(backend, first, "origin/master")).toBe("bbbbbbbb removes the declaration of backend, which cccccccc uses at Launch.kt:11")
    expect(describeLostBinding(rethrow, first, "origin/master")).toBe("bbbbbbbb removes the import of rethrow, which cccccccc uses at Launch.kt:12")
  })

  it("names the upstream when several of its commits change the file", () => {
    const first = conflict([commit("c", "Start with the backend", ["src/Launch.kt"]), commit("d", "Reformat", ["src/Launch.kt"])])
    expect(describeLostBinding(backend, first, "origin/master")).toBe("bbbbbbbb removes the declaration of backend, which origin/master uses at Launch.kt:11")
  })
})

describe("lostBindingsOf", () => {
  it("is empty for a conflict of git", () => {
    const { lostBindings: _, ...ofGit } = conflict([])
    expect(lostBindingsOf(ofGit)).toEqual([])
    expect(lostBindingsOf(conflict([]))).toEqual([backend, rethrow])
  })
})
