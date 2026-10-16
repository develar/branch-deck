import { describe, expect, it } from "vite-plus/test"
import { AISettingsSchema } from "@/stores/settings/aiSettings"

describe("AISettingsSchema", () => {
  it("asks again after the consent to the on-device model", () => {
    expect(AISettingsSchema.parse({ aiMode: "enabled" })).toEqual({ aiMode: "initial" })
  })

  it("keeps the other answers", () => {
    expect(AISettingsSchema.parse({ aiMode: "claude" })).toEqual({ aiMode: "claude" })
    expect(AISettingsSchema.parse({ aiMode: "disabled" })).toEqual({ aiMode: "disabled" })
  })

  it("starts with the prompt", () => {
    expect(AISettingsSchema.parse({})).toEqual({ aiMode: "initial" })
  })
})
