import { describe, it, expect } from "vitest"
import { mountSuspended } from "@nuxt/test-utils/runtime"
import type { CommitDetail } from "~/utils/bindings"
import UnassignedCommitListCard from "./UnassignedCommitListCard.vue"

describe("UnassignedCommitListCard", () => {
  const mockCommits: CommitDetail[] = [
    {
      hash: "abc123",
      originalHash: "def456",
      subject: "Fix bug without prefix",
      message: "Fix bug without prefix",
      author: "John Doe",
      authorTime: 1234567890,
      committerTime: 1234567890,
      status: "Pending",
      error: null,
    },
    {
      hash: "ghi789",
      originalHash: "jkl012",
      subject: "Add feature without prefix",
      message: "Add feature without prefix",
      author: "Jane Smith",
      authorTime: 1234567891,
      committerTime: 1234567891,
      status: "Pending",
      error: null,
    },
  ]

  it("renders the card with correct title", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: mockCommits,
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
    })

    // Check if the card is rendered
    expect(wrapper.find(".overflow-hidden").exists()).toBe(true)

    // Check if CardHeader is rendered with correct props
    const cardHeaderText = wrapper.text()
    expect(cardHeaderText).toContain("Unassigned Commits")
    expect(cardHeaderText).toContain("2 commits")
  })

  it("renders empty state with no commits", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: [],
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
    })

    const cardHeaderText = wrapper.text()
    expect(cardHeaderText).toContain("Unassigned Commits")
    expect(cardHeaderText).toContain("0 commits")
  })

  it("renders single commit with correct singular form", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: [mockCommits[0]!],
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
    })

    const cardHeaderText = wrapper.text()
    expect(cardHeaderText).toContain("1 commit")
  })

  it("passes commits to CommitList component", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: mockCommits,
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
    })

    // Check if commits are displayed
    expect(wrapper.text()).toContain("Fix bug without prefix")
    expect(wrapper.text()).toContain("Add feature without prefix")
  })

  it("shows author names in the commit list", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: mockCommits,
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
    })

    // Check if author names are displayed
    expect(wrapper.text()).toContain("John Doe")
    expect(wrapper.text()).toContain("Jane Smith")
  })

  it("displays the help popover with unassigned commits info", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: mockCommits,
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
      global: {
        stubs: {
          UnassignedCommitsHelpPopover: {
            template: `<div class="unassigned-help-popover">
              <div>About unassigned commits</div>
              <div>These commits won't be assigned to virtual branches</div>
            </div>`,
          },
        },
      },
    })

    // Check if the UnassignedCommitsHelpPopover is rendered
    expect(wrapper.text()).toContain("About unassigned commits")
    expect(wrapper.text()).toContain("These commits won't be assigned to virtual branches")
  })

  it("uses neutral badge color for commit count", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: mockCommits,
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
      global: {
        stubs: {
          CardHeader: {
            props: ["title", "count", "itemSingular", "itemPlural", "badgeColor"],
            template: `<div>{{ title }} - {{ count }} {{ count === 1 ? itemSingular : itemPlural }} - Badge: {{ badgeColor }}</div>`,
          },
        },
      },
    })

    expect(wrapper.text()).toContain("Badge: neutral")
  })

  it("uses compact variant for CommitList", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: mockCommits,
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
      global: {
        stubs: {
          CommitList: {
            props: ["commits", "variant", "showAuthor"],
            template: `<div>Variant: {{ variant }}, Show Author: {{ showAuthor }}</div>`,
          },
        },
      },
    })

    expect(wrapper.text()).toContain("Variant: compact")
    expect(wrapper.text()).toContain("Show Author: true")
  })

  it("selection actions are in the card header", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: mockCommits,
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
      global: {
        stubs: {
          CardHeader: {
            props: ["title", "count", "itemSingular", "itemPlural", "badgeColor"],
            template: `<div class="card-header-stub">
              {{ title }} - {{ count }} {{ count === 1 ? itemSingular : itemPlural }}
              <div class="actions-slot"><slot name="actions" /></div>
            </div>`,
          },
          CommitList: {
            props: ["commits", "variant", "showAuthor", "selectable"],
            emits: ["selection-change"],
            setup(_props, { expose }) {
              // Mock the exposed table with selected items
              const table = {
                getSelectedRowModel: () => ({
                  rows: mockCommits.map((commit, index) => ({
                    id: commit.hash,
                    index,
                    original: commit,
                  })),
                }),
                resetRowSelection: () => {},
              }

              expose({ table })
              return { table }
            },
            template: `<div ref="el">CommitList Mock</div>`,
          },
          FloatingSelectionBar: {
            props: ["selectedCount", "targetElement", "isInlineCreationActive"],
            template: `<div class="floating-selection-bar">
              {{ selectedCount }}
              <button>Group into Branch</button>
            </div>`,
          },
          InlineBranchCreator: {
            template: `<div>Inline Creator</div>`,
          },
          SelectionHelpPopover: {
            template: `<div>Selection Help</div>`,
          },
        },
      },
    })

    // Check that the floating selection bar contains selection actions
    const floatingBar = wrapper.find(".floating-selection-bar")
    expect(floatingBar.exists()).toBe(true)

    // When items are selected, selection count and actions should appear
    await wrapper.vm.$nextTick()

    // Check for selected count and group into branch button in the floating selection bar
    expect(wrapper.text()).toContain("2")
    expect(wrapper.text()).toContain("Group into Branch")
  })

  it("shows help popover with selection shortcuts when commits are available", async () => {
    const wrapper = await mountSuspended(UnassignedCommitListCard, {
      props: {
        commits: mockCommits,
        repositoryPath: "/test/repo",
        branchPrefix: "test-user",
      },
      global: {
        stubs: {
          CommitList: {
            props: ["commits", "variant", "showAuthor", "selectable"],
            setup() {
              // Mock the exposed table with no selection
              const table = {
                getSelectedRowModel: () => ({
                  rows: [],
                }),
                resetRowSelection: () => {},
              }
              return { table }
            },
            template: `<div ref="el">CommitList Mock</div>`,
          },
          UnassignedCommitsHelpPopover: {
            template: `<div class="unassigned-help-popover">Selection shortcuts</div>`,
          },
        },
      },
    })

    // UnassignedCommitsHelpPopover should be visible and contain selection help
    const helpPopover = wrapper.find(".unassigned-help-popover")
    expect(helpPopover.exists()).toBe(true)
    expect(wrapper.text()).toContain("Selection shortcuts")
  })
})