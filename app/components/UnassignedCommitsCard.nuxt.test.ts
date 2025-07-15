import { describe, it, expect } from "vitest"
import { mountSuspended } from "@nuxt/test-utils/runtime"
import type { CommitDetail } from "~/utils/bindings"
import UnassignedCommitsCard from "./UnassignedCommitsCard.vue"

describe("UnassignedCommitsCard", () => {
  const mockCommits: CommitDetail[] = [
    {
      hash: "abc123",
      originalHash: "def456",
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
      message: "Add feature without prefix",
      author: "Jane Smith",
      authorTime: 1234567891,
      committerTime: 1234567891,
      status: "Pending",
      error: null,
    },
  ]

  it("renders the card with correct title", async () => {
    const wrapper = await mountSuspended(UnassignedCommitsCard, {
      props: {
        commits: mockCommits,
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
    const wrapper = await mountSuspended(UnassignedCommitsCard, {
      props: {
        commits: [],
      },
    })

    const cardHeaderText = wrapper.text()
    expect(cardHeaderText).toContain("Unassigned Commits")
    expect(cardHeaderText).toContain("0 commits")
  })

  it("renders single commit with correct singular form", async () => {
    const wrapper = await mountSuspended(UnassignedCommitsCard, {
      props: {
        commits: [mockCommits[0]!],
      },
    })

    const cardHeaderText = wrapper.text()
    expect(cardHeaderText).toContain("1 commit")
  })

  it("passes commits to CommitList component", async () => {
    const wrapper = await mountSuspended(UnassignedCommitsCard, {
      props: {
        commits: mockCommits,
      },
    })

    // Check if commits are displayed
    expect(wrapper.text()).toContain("Fix bug without prefix")
    expect(wrapper.text()).toContain("Add feature without prefix")
  })

  it("shows author names in the commit list", async () => {
    const wrapper = await mountSuspended(UnassignedCommitsCard, {
      props: {
        commits: mockCommits,
      },
    })

    // Check if author names are displayed
    expect(wrapper.text()).toContain("John Doe")
    expect(wrapper.text()).toContain("Jane Smith")
  })

  it("displays the info card in the footer", async () => {
    const wrapper = await mountSuspended(UnassignedCommitsCard, {
      props: {
        commits: mockCommits,
      },
    })

    // Check if the info card is rendered
    expect(wrapper.text()).toContain("Commits without prefix")
    expect(wrapper.text()).toContain("These commits don't have a prefix in parentheses")
  })

  it("uses neutral badge color for commit count", async () => {
    const wrapper = await mountSuspended(UnassignedCommitsCard, {
      props: {
        commits: mockCommits,
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
    const wrapper = await mountSuspended(UnassignedCommitsCard, {
      props: {
        commits: mockCommits,
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
})