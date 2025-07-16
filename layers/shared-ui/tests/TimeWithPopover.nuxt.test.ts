import { describe, it, expect, vi } from "vitest"
import { mountSuspended } from "@nuxt/test-utils/runtime"
import TimeWithPopover from "./TimeWithPopover.vue"

// Mock the time formatting utility
vi.mock("~/utils/time", () => ({
  formatTimestamp: (time: number) => {
    // Return a consistent format for testing
    if (time === 1234567890) return "Feb 13, 2009 11:31 PM"
    if (time === 1234567900) return "Feb 13, 2009 11:31 PM"
    if (time === 0) return "Jan 1, 1970 12:00 AM"
    if (time === 2524608000) return "Jan 1, 2050 12:00 AM"
    return `Timestamp: ${time}`
  },
}))

describe("TimeWithPopover", () => {
  const mockAuthorTime = 1234567890 // Feb 13, 2009
  const mockCommitterTime = 1234567900 // 10 seconds later

  describe("when author time equals committer time", () => {
    it("renders only timestamp without popover", async () => {
      const wrapper = await mountSuspended(TimeWithPopover, {
        props: {
          authorTime: mockAuthorTime,
          committerTime: mockAuthorTime,
        },
      })

      // Should render plain span, not UPopover
      expect(wrapper.find("span").exists()).toBe(true)
      expect(wrapper.text()).toContain("Feb 13, 2009 11:31 PM") // Formatted timestamp

      // Should not have popover component
      const popover = wrapper.findComponent({ name: "UPopover" })
      expect(popover.exists()).toBe(false)
    })
  })

  describe("when committer time is not provided", () => {
    it("renders only timestamp without popover", async () => {
      const wrapper = await mountSuspended(TimeWithPopover, {
        props: {
          authorTime: mockAuthorTime,
        },
      })

      // Should render plain span
      expect(wrapper.find("span").exists()).toBe(true)
      expect(wrapper.text()).toContain("Feb 13, 2009 11:31 PM")

      // Should not have popover component
      const popover = wrapper.findComponent({ name: "UPopover" })
      expect(popover.exists()).toBe(false)
    })
  })

  describe("when author time differs from committer time", () => {
    it("renders timestamp with popover", async () => {
      const wrapper = await mountSuspended(TimeWithPopover, {
        props: {
          authorTime: mockAuthorTime,
          committerTime: mockCommitterTime,
        },
      })

      // Should have popover component
      const popover = wrapper.findComponent({ name: "UPopover" })
      expect(popover.exists()).toBe(true)

      // Popover should be in hover mode
      expect(popover.props("mode")).toBe("hover")

      // Should show author time as main text
      expect(wrapper.text()).toContain("Feb 13, 2009 11:31 PM")
    })

    it("popover content shows both authored and committed times", async () => {
      const wrapper = await mountSuspended(TimeWithPopover, {
        props: {
          authorTime: mockAuthorTime,
          committerTime: mockCommitterTime,
        },
        global: {
          stubs: {
            UPopover: {
              props: ["mode"],
              slots: ["default", "content"],
              template: `
                <div>
                  <slot />
                  <div class="popover-content">
                    <slot name="content" />
                  </div>
                </div>
              `,
            },
          },
        },
      })

      // Check popover content
      const popoverContent = wrapper.find(".popover-content")
      expect(popoverContent.exists()).toBe(true)

      // Should have table with both times
      const table = popoverContent.find("table")
      expect(table.exists()).toBe(true)
      expect(table.classes()).toContain("text-xs")

      // Check for "Authored" row
      const rows = table.findAll("tr")
      expect(rows).toHaveLength(2)

      const authoredRow = rows[0]
      expect(authoredRow).toBeDefined()
      expect(authoredRow!.find("td").text()).toBe("Authored")
      const authoredCells = authoredRow!.findAll("td")
      expect(authoredCells[1]?.text()).toContain("Feb 13, 2009 11:31 PM")

      const committedRow = rows[1]
      expect(committedRow).toBeDefined()
      expect(committedRow!.find("td").text()).toBe("Committed")
      const committedCells = committedRow!.findAll("td")
      expect(committedCells[1]?.text()).toContain("Feb 13, 2009 11:31 PM")
    })

    it("applies correct styling classes", async () => {
      const wrapper = await mountSuspended(TimeWithPopover, {
        props: {
          authorTime: mockAuthorTime,
          committerTime: mockCommitterTime,
        },
        global: {
          stubs: {
            UPopover: {
              props: ["mode"],
              slots: ["default", "content"],
              template: `
                <div>
                  <slot />
                  <div class="popover-content">
                    <slot name="content" />
                  </div>
                </div>
              `,
            },
          },
        },
      })

      const popoverContent = wrapper.find(".popover-content")

      // Check padding on content wrapper
      const contentWrapper = popoverContent.find("div.p-3")
      expect(contentWrapper.exists()).toBe(true)

      // Check label styling
      const labels = popoverContent.findAll("td.text-muted")
      expect(labels).toHaveLength(2)
      labels.forEach((label) => {
        expect(label.classes()).toContain("pr-3")
      })

      // Check value styling
      const values = popoverContent.findAll("td.text-highlighted")
      expect(values).toHaveLength(2)
    })
  })

  describe("edge cases", () => {
    it("handles very old timestamps correctly", async () => {
      const oldTime = 0 // Unix epoch
      const wrapper = await mountSuspended(TimeWithPopover, {
        props: {
          authorTime: oldTime,
          committerTime: oldTime + 100,
        },
      })

      expect(wrapper.text()).toContain("Jan 1, 1970")
    })

    it("handles future timestamps correctly", async () => {
      const futureTime = 2524608000 // Year 2050
      const wrapper = await mountSuspended(TimeWithPopover, {
        props: {
          authorTime: futureTime,
        },
      })

      expect(wrapper.text()).toContain("Jan 1, 2050")
    })
  })
})