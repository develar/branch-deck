import { ref, onMounted, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'

// Handle ESC key to close sub-window - set up immediately
const handleEscape = async (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    const currentWindow = getCurrentWindow()
    // Only close if not the main window
    if (currentWindow.label !== 'main') {
      await currentWindow.close()
    }
  }
}

// Add ESC handler immediately when module loads (for sub-windows only)
if (typeof window !== 'undefined' && getCurrentWindow().label !== 'main') {
  window.addEventListener('keydown', handleEscape)
}

export function useSubWindowData<T>() {
  const data = ref<T | null>(null)

  onMounted(() => {
    // Function to load data from global variable
    const loadData = () => {
      if (window.__INIT_DATA__) {
        data.value = window.__INIT_DATA__ as T
      }
    }

    // Load initial data
    loadData()
    
    // Listen for data updates when window is refocused with new data
    const handleDataUpdate = () => {
      loadData()
    }
    
    window.addEventListener('init-data-updated', handleDataUpdate)
    
    // Clean up on unmount
    onUnmounted(() => {
      window.removeEventListener('init-data-updated', handleDataUpdate)
      // Note: We don't remove the ESC handler here since it should persist
      // for the lifetime of the window, not just the component
    })
  })

  return data
}