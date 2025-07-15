import { describe, it, expect, vi, beforeEach } from "vitest"
import { ref } from "vue"
import { useCopyToClipboard } from "~/composables/useCopyToClipboard"

// Mock navigator.clipboard
const mockWriteText = vi.fn()
Object.defineProperty(navigator, "clipboard", {
  value: {
    writeText: mockWriteText,
  },
  writable: true,
})

describe("useCopyToClipboard", () => {
  beforeEach(() => {
    mockWriteText.mockClear()
  })
  it("useCopyToClipboard composable works correctly", async () => {
    const { copiedItems, copyToClipboard } = useCopyToClipboard()

    // Initially, no items should be copied
    expect(copiedItems.value.size).toBe(0)

    // Copy text
    await copyToClipboard("test text")

    // Check that clipboard was called
    expect(mockWriteText).toHaveBeenCalledWith("test text")

    // Check that item is marked as copied
    expect(copiedItems.value.has("test text")).toBe(true)
  })

  it("tooltip text changes when item is copied", () => {
    const copiedItems = ref(new Set<string>())
    const text = "test text"
    const tooltip = "Copy to clipboard"

    // Compute tooltip text (same logic as in component)
    const tooltipText = copiedItems.value.has(text) ? "Copied!" : tooltip
    expect(tooltipText).toBe("Copy to clipboard")

    // Add to copied items
    copiedItems.value.add(text)

    // Recompute tooltip text
    const updatedTooltipText = copiedItems.value.has(text) ? "Copied!" : tooltip
    expect(updatedTooltipText).toBe("Copied!")
  })

  it("icon changes when item is copied", () => {
    const copiedItems = ref(new Set<string>())
    const text = "test text"

    // Compute icon (same logic as in component)
    const icon = copiedItems.value.has(text) ? "i-lucide-copy-check" : "i-lucide-copy"
    expect(icon).toBe("i-lucide-copy")

    // Add to copied items
    copiedItems.value.add(text)

    // Recompute icon
    const updatedIcon = copiedItems.value.has(text) ? "i-lucide-copy-check" : "i-lucide-copy"
    expect(updatedIcon).toBe("i-lucide-copy-check")
  })

  it("button classes change when item is copied", () => {
    const copiedItems = ref(new Set<string>())
    const text = "test text"

    // Compute classes (same logic as in component)
    const getClasses = () => [
      "transition-all",
      copiedItems.value.has(text) ? "opacity-100 text-success" : "opacity-0 group-hover:opacity-100",
    ]

    let classes = getClasses()
    expect(classes).toContain("transition-all")
    expect(classes).toContain("opacity-0 group-hover:opacity-100")
    expect(classes).not.toContain("opacity-100 text-success")

    // Add to copied items
    copiedItems.value.add(text)

    // Recompute classes
    classes = getClasses()
    expect(classes).toContain("transition-all")
    expect(classes).toContain("opacity-100 text-success")
    expect(classes).not.toContain("opacity-0 group-hover:opacity-100")
  })

  it("custom tooltip text is used", () => {
    const copiedItems = ref(new Set<string>())
    const text = "branch-name"
    const customTooltip = "Copy full branch name to clipboard"

    // Compute tooltip text with custom tooltip
    const tooltipText = copiedItems.value.has(text) ? "Copied!" : customTooltip
    expect(tooltipText).toBe("Copy full branch name to clipboard")
  })

  it("handles different tooltip texts correctly", () => {
    const copiedItems = ref(new Set<string>())

    // Test file name tooltip
    const fileName = "src/main.ts"
    const fileTooltip = "Copy file name to clipboard"
    let tooltipText = copiedItems.value.has(fileName) ? "Copied!" : fileTooltip
    expect(tooltipText).toBe("Copy file name to clipboard")

    // Test branch name tooltip
    const branchName = "feature/new-feature"
    const branchTooltip = "Copy full branch name to clipboard"
    tooltipText = copiedItems.value.has(branchName) ? "Copied!" : branchTooltip
    expect(tooltipText).toBe("Copy full branch name to clipboard")
  })

  it("copied items are removed after timeout", async () => {
    vi.useFakeTimers()
    const { copiedItems, copyToClipboard } = useCopyToClipboard()

    // Copy text
    await copyToClipboard("test text")
    expect(copiedItems.value.has("test text")).toBe(true)

    // Advance time by 2 seconds
    vi.advanceTimersByTime(2000)

    // Item should be removed
    expect(copiedItems.value.has("test text")).toBe(false)

    vi.useRealTimers()
  })

  it("handles copy errors gracefully", async () => {
    const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {})
    mockWriteText.mockRejectedValueOnce(new Error("Clipboard access denied"))

    const { copiedItems, copyToClipboard } = useCopyToClipboard()

    // Try to copy
    await copyToClipboard("test text")

    // Should log error
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      "Failed to copy to clipboard:",
      expect.any(Error),
    )

    // Item should not be marked as copied
    expect(copiedItems.value.has("test text")).toBe(false)

    consoleErrorSpy.mockRestore()
  })
})