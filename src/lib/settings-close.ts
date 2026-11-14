import { runAppCommand } from "@/lib/app-commands"
import { describeError, logWarning } from "@/lib/errors"
import type { WriteFailure } from "@/lib/field-writes"
import type { SettingsTarget } from "@/lib/settings-window"

/*
 * The fields of Settings that hold valid typing save it before the window closes (⌘W, the close button, Escape outside
 * the field): one listener of the window (`SettingsPage`) waits for every save a field registered here, since a second
 * listener would close the window as soon as the first one answered. Each save sends its value at once, beside a write
 * of the field that still runs (`FieldWrites`, `now`). The close waits at most `CLOSE_SAVE_LIMIT_MS`: a write the backend
 * never answers keeps no window open. The main window, which outlives the window, tells a save that failed by the title
 * of the failure of its write, and one still without an answer by then as progress, since Rust may still apply it; each
 * with Open Settings on the field (`settings_not_saved`, openspec/specs/repository/settings/spec.md).
 */

/** How long the close of the window waits for the saves of its fields */
export const CLOSE_SAVE_LIMIT_MS = 3000

/**
 * A field whose save the close may not finish: the title of the failure of its write ("Cannot change …"), the title of
 * that write as progress ("Changing …"), and where the field is
 */
export interface SettingNotSaved {
  readonly title: string
  readonly pendingTitle: string
  readonly target: SettingsTarget
}

/** What the main window tells of a save the close of Settings did not finish (`settings_not_saved`) */
export interface SettingsNotSavedMessage {
  /** `failed`: the write failed, nothing was written; `pending`: no answer came in time, Rust may still write it */
  readonly kind: "failed" | "pending"
  readonly title: string
  /** Why: the error of the write, or that no answer came in time */
  readonly description: string
  readonly target: SettingsTarget
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
      const { title, target } = entry.notSaved()
      tellNotSaved({ kind: "failed", title, description: describeError(error), target })
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
      logWarning("The settings were still saving as the window closed", `no answer in ${CLOSE_SAVE_LIMIT_MS} ms`)
      for (const entry of pending) {
        const { pendingTitle, target } = entry.notSaved()
        const description = `Branch Deck has not answered in ${CLOSE_SAVE_LIMIT_MS / 1000} seconds and may still save the change`
        tellNotSaved({ kind: "pending", title: pendingTitle, description, target })
      }
      resolve()
    }, CLOSE_SAVE_LIMIT_MS)
  })
  return Promise.race([saved, limit]).finally(() => clearTimeout(timer))
}

/** The main window tells a save the close did not finish */
function tellNotSaved(message: SettingsNotSavedMessage): void {
  runAppCommand("settings_not_saved", message)
}
