// @spec spec/repository/settings.spec.md
import { describe, expect, it } from "vite-plus/test"
import { APP_SETTINGS_DEFAULTS } from "@/lib/bindings"
import { appSettingsStore } from "@/stores/settings/appSettings"

describe("app settings", () => {
  it("takes the defaults of the fields Rust reads from Rust, so a value back at its default means the same to both", () => {
    // no snapshot outside a window: every field is its default
    const settings = appSettingsStore.getState()
    const rustFields = Object.fromEntries(Object.keys(APP_SETTINGS_DEFAULTS).map(field => [field, settings[field as keyof typeof settings]]))
    expect(rustFields).toEqual(APP_SETTINGS_DEFAULTS)
    // Rust has no app prefix by default (`AppSettings::to_full` leaves the field out)
    expect(settings.globalUserBranchPrefix).toBeUndefined()
  })
})
