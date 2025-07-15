import { describe, it, expect } from "vitest"
import { mountSuspended } from "@nuxt/test-utils/runtime"
import type { CommitDetail, CommitSyncStatus } from "~/utils/bindings"
import CommitList from "./CommitList.vue"

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

      expect(wrapper.findAll(".px-6")).toHaveLength(1)
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

      expect(wrapper.findAll(".px-6")).toHaveLength(1)
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

      const commitItem = wrapper.find(".px-6")
      expect(commitItem.classes()).toContain("hover:bg-muted")
    })

    it("hides hover effect when showHover is false", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          showHover: false,
        },
      })

      const commitItem = wrapper.find(".px-6")
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

  describe("custom classes", () => {
    it("applies custom container class", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          containerClass: "custom-container",
        },
      })

      expect(wrapper.find(".custom-container").exists()).toBe(true)
    })

    it("applies custom item class", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          itemClass: "custom-item",
        },
      })

      expect(wrapper.find(".custom-item").exists()).toBe(true)
    })

    it("applies custom message class", async () => {
      const wrapper = await mountSuspended(CommitList, {
        props: {
          commits: mockCommits,
          messageClass: "custom-message",
        },
      })

      expect(wrapper.find(".custom-message").exists()).toBe(true)
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
})