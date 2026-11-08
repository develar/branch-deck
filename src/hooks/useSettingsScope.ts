import { createContext, useContext } from "react"
import type { SettingsScope } from "@/lib/settings-window"

/** The scope a page of the git config shows (its tab), and the repository it reads: `path` empty without one */
export interface SettingsScopeValue {
  readonly scope: SettingsScope
  /** Shows the other tab, as Open <repository> of a row does */
  readonly setScope: (scope: SettingsScope) => void
  readonly path: string
  /** The name of the repository, empty without one */
  readonly name: string
}

export const SettingsScopeContext = createContext<SettingsScopeValue>({ scope: "all", setScope: () => {}, path: "", name: "" })

/** The scope of the page of the git config this row is on (`SettingsScopeContext` of `SettingsPage`) */
export function useSettingsScope(): SettingsScopeValue {
  return useContext(SettingsScopeContext)
}
