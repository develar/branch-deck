import ColorSelectorModal from "~/components/ColorSelectorModal.vue"
import { useAppSettingsStore } from "~/stores/appSettings"

export function useColorSelector() {
  const appConfig = useAppConfig()

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
