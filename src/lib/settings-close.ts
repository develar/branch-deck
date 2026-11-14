import { runAppCommand } from "@/lib/app-commands"
import { describeError, logError } from "@/lib/errors"
import type { WriteFailure } from "@/lib/field-writes"
import type { SettingsTarget } from "@/lib/settings-window"

/*
 * The fields of Settings that hold valid typing save it before the window closes (⌘W, the close button, Escape outside
 * the field): one listener of the window (`SettingsPage`) waits for every save a field registered here, since a second
 * listener would close the window as soon as the first one answered. Each save sends its value at once, beside a write
 * of the field that still runs (`FieldWrites`, `now`). The close waits at most `CLOSE_SAVE_LIMIT_MS`: a write the backend
 * never answers keeps no window open. A save that failed or did not answer by then is told by the main window, which
 * outlives the window (`settings_not_saved`): the title of the failure of its write, and Open Settings on the field
 * (openspec/specs/repository/settings/spec.md, "The close saves the typing").
 */

/** How long the close of the window waits for the saves of its fields */
export const CLOSE_SAVE_LIMIT_MS = 3000

/** A field whose save the close could not finish: the title of the failure of its write, and where the field is */
export interface SettingNotSaved {
  readonly title: string
  readonly target: SettingsTarget
}

/** What the main window tells of a save the close of Settings could not finish (`settings_not_saved`) */
export interface SettingsNotSavedMessage extends SettingNotSaved {
  /** Why: the error of the write, or that no answer came in time */
  readonly description: string
}

interface CloseSave {
  readonly save: () => Promise<WriteFailure | null>
  readonly notSaved: () => SettingNotSaved
}

const saves = new Set<CloseSave>()

/** Registers the save of a field and what the main window says when it cannot finish; returns its removal */
export function saveBeforeClose(save: () => Promise<WriteFailure | null>, notSaved: () => SettingNotSaved): () => void {
  const entry: CloseSave = { save, notSaved }
  saves.add(entry)
  return () => {
    saves.delete(entry)
  }
}

/**
 * Every registered save, for the close of the window: resolves once all ended, or after `CLOSE_SAVE_LIMIT_MS`; each that
 * failed, and each that has not answered by then, is told by the main window
 */
export function saveAllBeforeClose(): Promise<void> {
  const pending = new Set(saves)
  const ended = (entry: CloseSave, error: unknown) => {
    pending.delete(entry)
    if (error !== null) {
      tellNotSaved(entry, describeError(error))
    }
  }
  const saved = Promise.all(
    [...saves].map(entry =>
      entry.save().then(
        failure => ended(entry, failure === null ? null : failure.failed),
        (error: unknown) => ended(entry, error),
      ),
    ),
  ).then(() => undefined)
  let timer: ReturnType<typeof setTimeout> | undefined
  const limit = new Promise<void>(resolve => {
    timer = setTimeout(() => {
      logError("Cannot save the settings before the window closes", `no answer in ${CLOSE_SAVE_LIMIT_MS} ms`)
      for (const entry of pending) {
        tellNotSaved(entry, `Branch Deck did not answer within ${CLOSE_SAVE_LIMIT_MS / 1000} seconds`)
      }
      resolve()
    }, CLOSE_SAVE_LIMIT_MS)
  })
  return Promise.race([saved, limit]).finally(() => clearTimeout(timer))
}

/** The main window tells that the save of `entry` did not finish, `description` why */
function tellNotSaved(entry: CloseSave, description: string): void {
  const message: SettingsNotSavedMessage = { ...entry.notSaved(), description }
  runAppCommand("settings_not_saved", message)
}
