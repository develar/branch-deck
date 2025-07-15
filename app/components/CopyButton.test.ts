import { describe, it, expect, vi, beforeEach } from "vitest"
import { mount } from "@vue/test-utils"
import { ref } from "vue"
import CopyButton from "./CopyButton.vue"

// Mock the composable
vi.mock("~/composables/useCopyToClipboard", () => ({
  useCopyToClipboard: () => {
    const copiedItems = ref(new Set<string>())
    const copyToClipboard = vi.fn(async (text: string) => {
      copiedItems.value.add(text)
      setTimeout(() => {
        copiedItems.value.delete(text)
      }, 2000)
    })
    return { copiedItems, copyToClipboard }
  },
}))

describe("CopyButton", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("renders with default props", () => {
    const wrapper = mount(CopyButton, {
      props: {
        text: "test text",
      },
      global: {
        stubs: {
          UTooltip: {
            template: "<div :data-tooltip=\"text\"><slot /></div>",
            props: ["text"],
          },
          UButton: {
            template: "<button :class=\"$attrs.class\" @click=\"$emit('click', $event)\"><span v-if=\"icon\" :class=\"icon\" /></button>",
            props: ["icon", "size", "variant"],
          },
        },
      },
    })

    expect(wrapper.exists()).toBe(true)
  })

  it("shows correct icon based on copied state", async () => {
    const wrapper = mount(CopyButton, {
      props: {
        text: "test text",
      },
      global: {
        stubs: {
          UTooltip: {
            template: "<div :data-tooltip=\"text\"><slot /></div>",
            props: ["text"],
          },
          UButton: {
            template: "<button :class=\"$attrs.class\" @click=\"$emit('click', $event)\"><span v-if=\"icon\" :class=\"icon\" /></button>",
            props: ["icon", "size", "variant"],
          },
        },
      },
    })

    // Initially shows copy icon
    expect(wrapper.find("span").classes()).toContain("i-lucide-copy")

    // Click to copy
    await wrapper.find("button").trigger("click")
    await wrapper.vm.$nextTick()

    // Should show check icon after copying
    expect(wrapper.find("span").classes()).toContain("i-lucide-copy-check")
  })

  it("displays correct tooltip text", async () => {
    const wrapper = mount(CopyButton, {
      props: {
        text: "test text",
        tooltip: "Copy file name to clipboard",
      },
      global: {
        stubs: {
          UTooltip: {
            template: "<div :data-tooltip=\"text\"><slot /></div>",
            props: ["text"],
          },
          UButton: {
            template: "<button :class=\"$attrs.class\" @click=\"$emit('click', $event)\"><span v-if=\"icon\" :class=\"icon\" /></button>",
            props: ["icon", "size", "variant"],
          },
        },
      },
    })

    // Initially shows custom tooltip
    expect(wrapper.find("[data-tooltip]").attributes("data-tooltip")).toBe("Copy file name to clipboard")

    // Click to copy
    await wrapper.find("button").trigger("click")
    await wrapper.vm.$nextTick()

    // Should show "Copied!" after copying
    expect(wrapper.find("[data-tooltip]").attributes("data-tooltip")).toBe("Copied!")
  })

  it("applies correct button classes", async () => {
    const wrapper = mount(CopyButton, {
      props: {
        text: "test text",
      },
      global: {
        stubs: {
          UTooltip: {
            template: "<div :data-tooltip=\"text\"><slot /></div>",
            props: ["text"],
          },
          UButton: {
            template: "<button :class=\"$attrs.class\" @click=\"$emit('click', $event)\"><span v-if=\"icon\" :class=\"icon\" /></button>",
            props: ["icon", "size", "variant"],
          },
        },
      },
    })

    const button = wrapper.find("button")

    // Initially has hover opacity classes
    expect(button.classes()).toContain("transition-all")
    expect(button.classes()).toContain("opacity-0")
    expect(button.classes()).toContain("group-hover:opacity-100")

    // Click to copy
    await button.trigger("click")
    await wrapper.vm.$nextTick()

    // Should have success styling after copying
    expect(button.classes()).toContain("opacity-100")
    expect(button.classes()).toContain("text-success")
  })

  it("passes correct props to UButton", () => {
    const wrapper = mount(CopyButton, {
      props: {
        text: "test text",
        size: "md",
        variant: "solid",
      },
      global: {
        stubs: {
          UTooltip: {
            template: "<div :data-tooltip=\"text\"><slot /></div>",
            props: ["text"],
          },
          UButton: {
            template: "<button :size=\"size\" :variant=\"variant\"><slot /></button>",
            props: ["icon", "size", "variant"],
          },
        },
      },
    })

    const button = wrapper.find("button")
    expect(button.attributes("size")).toBe("md")
    expect(button.attributes("variant")).toBe("solid")
  })

  it("stops click event propagation", async () => {
    const parentClickHandler = vi.fn()

    const wrapper = mount({
      template: `
        <div @click="parentClickHandler">
          <CopyButton :text="text" />
        </div>
      `,
      components: { CopyButton },
      setup() {
        return {
          text: "test text",
          parentClickHandler,
        }
      },
    }, {
      global: {
        stubs: {
          UTooltip: {
            template: "<div :data-tooltip=\"text\"><slot /></div>",
            props: ["text"],
          },
          UButton: {
            template: "<button @click.stop=\"$emit('click', $event)\"><slot /></button>",
            props: ["icon", "size", "variant"],
          },
        },
      },
    })

    await wrapper.find("button").trigger("click")

    // Parent click handler should not be called due to .stop modifier
    expect(parentClickHandler).not.toHaveBeenCalled()
  })
})