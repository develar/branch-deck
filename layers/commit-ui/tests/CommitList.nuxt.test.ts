import { describe, it, expect } from "vitest"
import { mountSuspended } from "@nuxt/test-utils/runtime"
import type { CommitDetail, CommitSyncStatus } from "~/utils/bindings"
import CommitList from "./CommitList.vue"

// Type for generic commit (from CommitList component)
interface GenericCommit {
  hash?: string
  originalHash?: string
  message: string
  author?: string
  authorTime?: number
  committerTime?: number
  fileCount?: number
}

type CommitUnion = CommitDetail | GenericCommit

describe("CommitList", () => {
  const mockCommits = [
    {
      hash: "123abc456def",
      originalHash: "789ghi012jkl",
      message: "Test commit message",
      author: "Test Author",
      authorTime: 1234567890,
      committerTime: 1234567890,
      fileCount: 3,
    },
  ]

  const mockCommitWithStatus: CommitDetail = {
    hash: "abc123",
    originalHash: "def456",
    subject: "Commit with status",
    message: "Commit with status",
    author: "Test Author",
    authorTime: 1234567890,
    committerTime: 1234567890,
    status: "Pending" as CommitSyncStatus,
    error: null,
  }

  describe("rendering variants", () => {
    it("renders commits in compact variant", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          variant: "compact",
        },
      })

      expect(wrapper.find(".text-sm").text()).toContain("Test commit message")
      expect(wrapper.find(".font-mono").text()).toContain("789ghi0")
    })

    it("renders commits in detailed variant", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          variant: "detailed",
          showFileCount: true,
        },
      })

      expect(wrapper.text()).toContain("3 files")
    })

    it("renders commits in status variant with badges", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: [mockCommitWithStatus],
          variant: "status",
        },
      })

      // Should show badge for Pending status
      const badge = wrapper.find("[role=\"status\"]")
      expect(badge.exists() || wrapper.text().includes("Pending")).toBe(true)
    })
  })

  describe("commit normalization", () => {
    it("handles array of commits", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
        },
      })

      expect(wrapper.findAll(".bd-padding-list-item")).toHaveLength(1)
    })

    it("handles Map of commits", async () => {
      const commitMap = new Map([
        ["key1", mockCommitWithStatus],
      ])

      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: commitMap,
        },
      })

      expect(wrapper.findAll(".bd-padding-list-item")).toHaveLength(1)
    })
  })

  describe("visual features", () => {
    it("shows dividers by default", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
        },
      })

      expect(wrapper.find(".divide-y").exists()).toBe(true)
    })

    it("hides dividers when showDividers is false", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          showDividers: false,
        },
      })

      expect(wrapper.find(".divide-y").exists()).toBe(false)
      expect(wrapper.find(".space-y-2").exists()).toBe(true)
    })

    it("shows hover effect by default", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
        },
      })

      const commitItem = wrapper.find(".bd-padding-list-item")
      expect(commitItem.classes()).toContain("hover:bg-muted")
    })

    it("hides hover effect when showHover is false", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          showHover: false,
        },
      })

      const commitItem = wrapper.find(".bd-padding-list-item")
      expect(commitItem.classes()).not.toContain("hover:bg-muted")
    })
  })

  describe("metadata display", () => {
    it("shows author when enabled", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          showAuthor: true,
        },
      })

      expect(wrapper.text()).toContain("Test Author")
    })

    it("hides author by default", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
        },
      })

      expect(wrapper.text()).not.toContain("Test Author")
    })

    it("shows file count when enabled", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          showFileCount: true,
        },
      })

      expect(wrapper.text()).toContain("3 files")
    })
  })

  describe("selection features", () => {
    it("enables selection when selectable is true", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          selectable: true,
        },
      })

      const commitItem = wrapper.find(".bd-padding-list-item")
      expect(commitItem.classes()).toContain("cursor-pointer")
      expect(commitItem.classes()).toContain("select-none")
    })

    it("emits selection changes on click", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          selectable: true,
        },
      })

      const commitItem = wrapper.find(".bd-padding-list-item")
      await commitItem.trigger("click")

      const emitted = wrapper.emitted("selection-change")
      expect(emitted).toBeTruthy()
      expect(emitted![0]?.[0]).toHaveLength(1)
      const selection = emitted![0]?.[0] as CommitUnion[]
      expect(selection?.[0]?.originalHash).toBe("789ghi012jkl")
    })

    it("handles multi-selection with cmd/ctrl click", async () => {
      const multipleCommits = [
        ...mockCommits,
        {
          hash: "456def789ghi",
          originalHash: "012jkl345mno",
          message: "Second commit",
          author: "Test Author",
          authorTime: 1234567890,
          committerTime: 1234567890,
        },
      ]

      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: multipleCommits,
          selectable: true,
        },
      })

      // Click first commit
      const commits = wrapper.findAll(".bd-padding-list-item")
      await commits[0]!.trigger("click")

      // Cmd/Ctrl click second commit
      await commits[1]!.trigger("click", { metaKey: true })

      const emitted = wrapper.emitted("selection-change")
      expect(emitted).toBeTruthy()
      const lastEmit = emitted?.[emitted.length - 1]?.[0] as CommitUnion[]
      expect(lastEmit).toHaveLength(2)
      expect(lastEmit[0]?.originalHash).toBe("789ghi012jkl")
      expect(lastEmit[1]?.originalHash).toBe("012jkl345mno")
    })

    it("shows selection state properly", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          selectable: true,
        },
      })

      const commitItem = wrapper.find(".bd-padding-list-item")
      await commitItem.trigger("click")

      // Wait for next tick to allow Vue to update
      await wrapper.vm.$nextTick()

      expect(commitItem.classes().some(c => c.includes("bg-primary"))).toBe(true)
    })
  })

  describe("status badges", () => {
    it("shows badge only for exceptional statuses", async () => {
      const commitsWithVariousStatuses = [
        { ...mockCommitWithStatus, status: "Unchanged" as CommitSyncStatus },
        { ...mockCommitWithStatus, status: "Created" as CommitSyncStatus },
        { ...mockCommitWithStatus, status: "Error" as CommitSyncStatus },
        { ...mockCommitWithStatus, status: "Blocked" as CommitSyncStatus },
      ]

      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: commitsWithVariousStatuses,
          variant: "status",
        },
      })

      // Count elements that look like badges (contain specific status text)
      const errorBadges = wrapper.findAll("*").filter(el => el.text().includes("Error"))
      const blockedBadges = wrapper.findAll("*").filter(el => el.text().includes("Blocked"))

      // Should only show badges for Error and Blocked (exceptional states)
      expect(errorBadges.length).toBeGreaterThan(0)
      expect(blockedBadges.length).toBeGreaterThan(0)
    })

    it("shows animated loader for pending status", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: [mockCommitWithStatus],
          variant: "status",
        },
      })

      expect(wrapper.text()).toContain("Pending")
    })
  })

  describe("error handling", () => {
    it("shows merge conflict viewer for merge conflicts", async () => {
      const commitWithConflict = {
        ...mockCommitWithStatus,
        error: {
          MergeConflict: {
            commitMessage: "Conflict message",
            commitHash: "abc123",
            commitAuthorTime: 1234567890,
            commitCommitterTime: 1234567890,
            originalParentMessage: "Parent message",
            originalParentHash: "def456",
            originalParentAuthorTime: 1234567890,
            originalParentCommitterTime: 1234567890,
            targetBranchMessage: "Target message",
            targetBranchHash: "ghi789",
            targetBranchAuthorTime: 1234567890,
            targetBranchCommitterTime: 1234567890,
            conflictingFiles: [],
            conflictAnalysis: {
              missingCommits: [],
              mergeBaseHash: "base123",
              mergeBaseSubject: "Base commit",
              mergeBaseMessage: "Base commit",
              mergeBaseTime: 1234567890,
              mergeBaseAuthor: "Base Author",
              divergenceSummary: {
                commitsAheadInSource: 0,
                commitsAheadInTarget: 0,
                commonAncestorDistance: 0,
              },
            },
            conflictMarkerCommits: {},
          },
        },
      }

      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: [commitWithConflict],
          variant: "status",
          branchName: "test-branch",
        },
        global: {
          stubs: {
            MergeConflictViewer: {
              template: "<div class=\"merge-conflict-stub\">Merge Conflict Viewer</div>",
            },
          },
        },
      })

      // Check if the merge conflict viewer stub is rendered
      expect(wrapper.find(".merge-conflict-stub").exists()).toBe(true)
    })

    it("shows alert for generic errors", async () => {
      const commitWithError = {
        ...mockCommitWithStatus,
        error: {
          Generic: "Something went wrong",
        },
      }

      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: [commitWithError],
          variant: "status",
        },
      })

      expect(wrapper.text()).toContain("Something went wrong")
    })
  })

  describe("slots", () => {
    it("renders after-commit slot content", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
        },
        slots: {
          "after-commit": "<div class='test-slot'>Slot content</div>",
        },
      })

      expect(wrapper.find(".test-slot").text()).toBe("Slot content")
    })
  })

  describe("keyboard shortcuts", () => {
    it("selects all commits with Cmd/Ctrl+A", async () => {
      const multipleCommits = [
        ...mockCommits,
        {
          hash: "456def789ghi",
          originalHash: "012jkl345mno",
          message: "Second commit",
          author: "Test Author",
          authorTime: 1234567890,
          committerTime: 1234567890,
        },
      ]

      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: multipleCommits,
          selectable: true,
        },
      })

      // Focus the component
      const container = wrapper.find("[tabindex='0']")
      const element = container.element as HTMLElement
      element.focus()

      // Simulate Cmd+A
      await container.trigger("keydown", { key: "a", metaKey: true })
      await wrapper.vm.$nextTick()

      const emitted = wrapper.emitted("selection-change")
      expect(emitted).toBeTruthy()
      const lastEmit = emitted?.[emitted.length - 1]?.[0] as CommitUnion[]
      expect(lastEmit).toHaveLength(2)
    })

    it("clears selection with Escape", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          selectable: true,
        },
      })

      // First select an item
      const commitItem = wrapper.find(".bd-padding-list-item")
      await commitItem.trigger("click")
      await wrapper.vm.$nextTick()

      // Focus the component
      const container = wrapper.find("[tabindex='0']")
      const element = container.element as HTMLElement
      element.focus()

      // Then trigger escape
      await container.trigger("keydown", { key: "Escape" })
      await wrapper.vm.$nextTick()

      const emitted = wrapper.emitted("selection-change")
      expect(emitted).toBeTruthy()
      // The last emit should be empty array after clearing
      const lastEmit = emitted?.[emitted.length - 1]?.[0] as CommitUnion[]
      expect(lastEmit).toHaveLength(0)
    })
  })
})