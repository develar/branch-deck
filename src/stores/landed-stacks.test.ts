import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { forgetLandedStackRoot, getStackLandedBy, landedStacksStore, rememberLandedStackRoot } from "@/stores/landed-stacks"

const { repository } = vi.hoisted(() => ({ repository: { path: "/repo" } }))

vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: repository.path }),
  getEffectiveBranchPrefix: () => "user",
  useSelectedProjectPath: () => repository.path,
  useEffectiveBranchPrefix: () => "user",
}))

beforeEach(() => {
  landedStacksStore.setState(landedStacksStore.getInitialState(), true)
  repository.path = "/repo"
})

describe("forgetLandedStackRoot", () => {
  it("forgets the root in the repository the pull request was created in, also after a switch", () => {
    rememberLandedStackRoot("air", "auth")
    repository.path = "/other"
    rememberLandedStackRoot("air", "core")
    // the pull request of the whole air stack of /repo is created while /other is shown
    forgetLandedStackRoot("air", "/repo", "user")
    expect(getStackLandedBy("air")).toBe("core")
    repository.path = "/repo"
    expect(getStackLandedBy("air")).toBeNull()
  })
})
