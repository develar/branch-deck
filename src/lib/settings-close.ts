import { logError } from "@/lib/errors"

/*
 * The fields of Settings that hold valid typing save it before the window closes (⌘W, the close button, Escape outside
 * the field): one listener of the window (`SettingsPage`) waits for every save a field registered here, since a second
 * listener would close the window as soon as the first one answered. It waits at most `CLOSE_SAVE_LIMIT_MS`: a write the
 * backend never answers keeps no window open.
 */

/** How long the close of the window waits for the saves of its fields */
export const CLOSE_SAVE_LIMIT_MS = 3000

const saves = new Set<() => Promise<void>>()

/** Registers the save of a field; returns its removal */
export function saveBeforeClose(save: () => Promise<void>): () => void {
  saves.add(save)
  return () => {
    saves.delete(save)
  }
}

/** Every registered save, for the close of the window: resolves once all ended, or after `CLOSE_SAVE_LIMIT_MS` */
export function saveAllBeforeClose(): Promise<void> {
  const saved = Promise.all([...saves].map(save => save())).then(() => undefined)
  let timer: ReturnType<typeof setTimeout> | undefined
  const limit = new Promise<void>(resolve => {
    timer = setTimeout(() => {
      logError("Cannot save the settings before the window closes", `no answer in ${CLOSE_SAVE_LIMIT_MS} ms`)
      resolve()
    }, CLOSE_SAVE_LIMIT_MS)
  })
  return Promise.race([saved, limit]).finally(() => clearTimeout(timer))
}
