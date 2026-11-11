/*
 * A context menu opened from the keyboard, as the system opens one: by the context menu key and by Shift+F10.
 *
 * The engine's own `contextmenu` of such a key goes to the element with the focus, at a point of the engine's choosing
 * (the middle of that element, or the selection). That opens the menu of a trigger only when the focus is in the
 * trigger, and never at the place the menu is about, so a view that knows its place opens the menu itself.
 */

/** The fields of a key press that say whether it asks for the context menu */
interface MenuKey {
  readonly key: string
  readonly shiftKey: boolean
  readonly altKey: boolean
  readonly ctrlKey: boolean
  readonly metaKey: boolean
}

/** A press of a menu key, which `openMenuByKey` takes for itself */
interface MenuKeyPress {
  readonly key: string
  readonly repeat: boolean
  preventDefault: () => void
}

/** Whether a key press asks for the context menu: the context menu key, or Shift+F10 */
export function isMenuKey(event: MenuKey): boolean {
  return !event.altKey && !event.ctrlKey && !event.metaKey && (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey))
}

/**
 * Opens the context menu of `trigger` for the press of a menu key (`isMenuKey`), at a point of the window, as a right
 * click there does. The key opens nothing beside it: the press is prevented, which holds back the engine's menu of
 * Shift+F10 everywhere and of the context menu key off Windows. Windows opens the menu of that key as the key goes up,
 * at whatever has the focus by then (an item of the menu just opened), whether its press was prevented or not: the
 * release is prevented too, wherever it lands in `page`. A key held down opens the menu once.
 */
export function openMenuByKey(event: MenuKeyPress, trigger: EventTarget, clientX: number, clientY: number, page: EventTarget = window): void {
  event.preventDefault()
  if (event.repeat) {
    return
  }
  if (event.key === "ContextMenu") {
    preventRelease(page)
  }
  trigger.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX, clientY, button: 2 }))
}

/** The next release of the context menu key in `page` opens no menu; a window that lost the focus sees no release */
function preventRelease(page: EventTarget): void {
  const waiting = new AbortController()
  page.addEventListener(
    "keyup",
    event => {
      if ((event as KeyboardEvent).key === "ContextMenu") {
        event.preventDefault()
        waiting.abort()
      }
    },
    { capture: true, signal: waiting.signal },
  )
  // the blur of the window itself: the blur of an element (the focus goes into the menu) passes here on its way down
  page.addEventListener("blur", event => event.target === page && waiting.abort(), { capture: true, signal: waiting.signal })
}
