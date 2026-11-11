import { afterAll, beforeAll, describe, expect, it, vi } from "vite-plus/test"
import { isMenuKey, openMenuByKey } from "@/lib/menu-key"

/** The `contextmenu` the helper dispatches, as far as a test reads it */
class FakeMouseEvent extends Event {
  readonly clientX: number
  readonly clientY: number
  readonly button: number
  constructor(type: string, init: EventInit & { clientX: number; clientY: number; button: number }) {
    super(type, init)
    this.clientX = init.clientX
    this.clientY = init.clientY
    this.button = init.button
  }
}

/** A key event of the page, with its `key` */
function keyEvent(type: string, key: string): Event {
  return Object.assign(new Event(type, { cancelable: true }), { key })
}

function press(key: string, repeat = false) {
  return { key, repeat, preventDefault: vi.fn<() => void>() }
}

/** A trigger that records the menus opened on it */
function triggerOf(): { trigger: EventTarget; menus: FakeMouseEvent[] } {
  const trigger = new EventTarget()
  const menus: FakeMouseEvent[] = []
  trigger.addEventListener("contextmenu", event => menus.push(event as FakeMouseEvent))
  return { trigger, menus }
}

const NO_MODIFIER = { shiftKey: false, altKey: false, ctrlKey: false, metaKey: false }

beforeAll(() => {
  vi.stubGlobal("MouseEvent", FakeMouseEvent)
})

afterAll(() => {
  vi.unstubAllGlobals()
})

describe("menu key", () => {
  it("takes the context menu key and Shift+F10 for the menu, and no key with another modifier", () => {
    expect(isMenuKey({ ...NO_MODIFIER, key: "ContextMenu" })).toBe(true)
    expect(isMenuKey({ ...NO_MODIFIER, key: "F10", shiftKey: true })).toBe(true)
    // F10 alone opens the main menu
    expect(isMenuKey({ ...NO_MODIFIER, key: "F10" })).toBe(false)
    expect(isMenuKey({ ...NO_MODIFIER, key: "ContextMenu", ctrlKey: true })).toBe(false)
    expect(isMenuKey({ ...NO_MODIFIER, key: "F10", shiftKey: true, altKey: true })).toBe(false)
    expect(isMenuKey({ ...NO_MODIFIER, key: "ContextMenu", metaKey: true })).toBe(false)
    expect(isMenuKey({ ...NO_MODIFIER, key: "Enter" })).toBe(false)
  })

  it("opens the menu of the trigger at the point, as a right click there, and prevents the press", () => {
    const { trigger, menus } = triggerOf()
    const key = press("F10")
    openMenuByKey(key, trigger, 120, 48, new EventTarget())
    expect(key.preventDefault).toHaveBeenCalledOnce()
    expect(menus.map(menu => [menu.clientX, menu.clientY, menu.button, menu.bubbles, menu.cancelable])).toEqual([[120, 48, 2, true, true]])
  })

  it("prevents the release of the context menu key once, wherever it lands in the page, and no other release", () => {
    const { trigger } = triggerOf()
    const page = new EventTarget()
    openMenuByKey(press("ContextMenu"), trigger, 0, 0, page)
    // the release of another key meanwhile is the page's own
    const other = keyEvent("keyup", "Shift")
    page.dispatchEvent(other)
    expect(other.defaultPrevented).toBe(false)
    // Windows opens the menu of the key as it goes up: that release opens nothing
    const release = keyEvent("keyup", "ContextMenu")
    page.dispatchEvent(release)
    expect(release.defaultPrevented).toBe(true)
    // a later release, of a press the page did not take, is the page's own again
    const later = keyEvent("keyup", "ContextMenu")
    page.dispatchEvent(later)
    expect(later.defaultPrevented).toBe(false)
  })

  it("prevents no release after Shift+F10, whose menu the engine opens as the key goes down", () => {
    const { trigger } = triggerOf()
    const page = new EventTarget()
    openMenuByKey(press("F10"), trigger, 0, 0, page)
    const release = keyEvent("keyup", "ContextMenu")
    page.dispatchEvent(release)
    expect(release.defaultPrevented).toBe(false)
  })

  it("stops waiting for a release when the window loses the focus, not when an element of it does", () => {
    const { trigger } = triggerOf()
    const page = new EventTarget()
    openMenuByKey(press("ContextMenu"), trigger, 0, 0, page)
    // the blur of the element that had the focus, on its way down through the window: the focus went into the menu
    const element = new Event("blur")
    Object.defineProperty(element, "target", { value: new EventTarget() })
    page.dispatchEvent(element)
    const held = keyEvent("keyup", "ContextMenu")
    page.dispatchEvent(held)
    expect(held.defaultPrevented).toBe(true)

    openMenuByKey(press("ContextMenu"), trigger, 0, 0, page)
    page.dispatchEvent(new Event("blur"))
    const unseen = keyEvent("keyup", "ContextMenu")
    page.dispatchEvent(unseen)
    expect(unseen.defaultPrevented).toBe(false)
  })

  it("opens the menu once for a key held down", () => {
    const { trigger, menus } = triggerOf()
    const page = new EventTarget()
    openMenuByKey(press("ContextMenu"), trigger, 0, 0, page)
    const repeated = press("ContextMenu", true)
    openMenuByKey(repeated, trigger, 0, 0, page)
    // the repeat still opens no menu of the engine
    expect(repeated.preventDefault).toHaveBeenCalledOnce()
    expect(menus).toHaveLength(1)
  })
})
