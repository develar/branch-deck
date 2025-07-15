import { computed, onMounted, onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import type { IAppStore } from '~/utils/app-store'
import ColorSelectorModal from '~/components/ColorSelectorModal.vue'

export function useColorSelector(appStore: IAppStore) {
  const appConfig = useAppConfig()
  let unlisten: (() => void) | null = null
  
  // Create overlay instance
  const overlay = useOverlay()

  // Current settings with reactive getters/setters
  const primary = computed({
    get: () => appConfig.ui?.colors?.primary || 'blue',
    set: async (value: string) => {
      if (!appConfig.ui) appConfig.ui = {}
      if (!appConfig.ui.colors) appConfig.ui.colors = {}
      appConfig.ui.colors.primary = value
      await appStore.updateAppSetting('primaryColor', value)
    }
  })

  const gray = computed({
    get: () => appConfig.ui?.colors?.neutral || 'slate',
    set: async (value: string) => {
      if (!appConfig.ui) appConfig.ui = {}
      if (!appConfig.ui.colors) appConfig.ui.colors = {}
      appConfig.ui.colors.neutral = value
      await appStore.updateAppSetting('neutralColor', value)
    }
  })

  const radius = computed({
    get: () => appConfig.theme?.radius || 0.25,
    set: async (value: number) => {
      if (!appConfig.theme) {
        appConfig.theme = {}
      }
      appConfig.theme.radius = value
      await appStore.updateAppSetting('radius', value)
    }
  })

  // Load user preferences on mount
  onMounted(async () => {
    const settings = await appStore.getAppSettings()
    
    // Apply settings to appConfig
    if (!appConfig.ui) appConfig.ui = {}
    if (!appConfig.ui.colors) appConfig.ui.colors = {}
    if (!appConfig.theme) appConfig.theme = {}
    
    appConfig.ui.colors.primary = settings.primaryColor
    // appConfig.ui.colors.neutral = settings.neutralColor
    // appConfig.theme.radius = settings.radius

    // Listen for menu event to open color selector
    unlisten = await listen('open_color_selector', () => {
      openColorSelector()
    })
  })

  onUnmounted(() => {
    unlisten?.()
  })

  const openColorSelector = async () => {
    // Create modal with current settings as props
    const modal = overlay.create(ColorSelectorModal, {
      props: {
        currentColor: primary.value,
        currentNeutral: gray.value,
        currentRadius: radius.value,
        onChange: (setting: { type: string, value: string }) => {
          applyThemeSetting(setting)
        }
      }
    })

    // Open modal - it will stay open until user clicks outside
    modal.open()
  }

  const applyThemeSetting = async (setting: { type: string, value: string }) => {
    switch (setting.type) {
      case 'primary':
        primary.value = setting.value
        break
      case 'neutral':
        gray.value = setting.value
        break
      case 'radius':
        radius.value = parseFloat(setting.value)
        break
    }
  }

  return {
    primary,
    gray,
    radius,
    openColorSelector
  }
}