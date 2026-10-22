/*
 * Whether the user works with the keyboard now: the last input of the window was a key, not a pointer. An action that
 * opens a panel moves the focus into it only for the keyboard; a click leaves the focus where the pointer is.
 */

let keyboard = false

const onKey = (event: KeyboardEvent) => {
  // a modifier alone (⌘ before a click) is no keyboard work
  if (event.key !== "Meta" && event.key !== "Control" && event.key !== "Alt" && event.key !== "Shift") {
    keyboard = true
  }
}

const onPointer = () => {
  keyboard = false
}

/** Watches the inputs of the window; returns the stop */
export function watchInputModality(): () => void {
  document.addEventListener("keydown", onKey, { capture: true })
  document.addEventListener("pointerdown", onPointer, { capture: true })
  return () => {
    document.removeEventListener("keydown", onKey, { capture: true })
    document.removeEventListener("pointerdown", onPointer, { capture: true })
  }
}

/** The last input of the window was a key (`watchInputModality`) */
export function isKeyboardInput(): boolean {
  return keyboard
}
