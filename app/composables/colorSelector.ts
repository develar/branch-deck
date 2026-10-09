import { computed } from "vue"
import { useOverlay } from "@nuxt/ui/composables"
import { useAppConfig } from "@nuxt/ui/runtime/vue/composables/useAppConfig.js"
import ColorSelectorModal from "~/components/ColorSelectorModal.vue"
import { useAppSettingsStore } from "@layers/core/stores/appSettings"
import { scopedListen } from "~/utils/listen"

export function useColorSelector() {
  // reactive Nuxt UI app config; the colors plugin recomputes the `--ui-color-*` variables from it
  const appConfig = useAppConfig() as { ui: { colors: Record<string, string> } }

  const appSettingsStore = useAppSettingsStore()
  appConfig.ui.colors.primary = appSettingsStore.primaryColor

  const overlay = useOverlay()

  scopedListen("open_color_selector", () => {
    const primary = computed({
      get: () => appConfig.ui.colors.primary,
      set: async (value: string) => {
        const appSettingsStore = useAppSettingsStore()
        appConfig.ui.colors.primary = value
        appSettingsStore.primaryColor = value
      },
    })

    // Create modal with current settings as props
    const modal = overlay.create(ColorSelectorModal, {
      props: {
        currentColor: primary.value,
        // currentNeutral: gray.value,
        // currentRadius: radius.value,
        onChange: (setting: { type: string, value: string }) => {
          switch (setting.type) {
            case "primary":
              primary.value = setting.value
              break
            case "neutral":
              // gray.value = setting.value
              break
            case "radius":
              // radius.value = parseFloat(setting.value)
              break
          }
        },
      },
    })

    modal.open()
  })
}
