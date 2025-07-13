import { ref, inject, watch, onMounted } from 'vue'
import { appStoreKey } from '~/utils/app-store'

export function useConflictViewerSettings() {
  const appStore = inject(appStoreKey)
  if (!appStore) {
    throw new Error('AppStore not provided')
  }

  // Reactive settings
  const showConflictsOnly = ref(true)
  const viewMode = ref('diff')
  const conflictDiffViewMode = ref<'unified' | 'split'>('unified')

  // Load settings from store on mount
  onMounted(async () => {
    const settings = await appStore.getConflictViewerSettings()
    showConflictsOnly.value = settings.showConflictsOnly
    viewMode.value = settings.viewMode
    conflictDiffViewMode.value = settings.conflictDiffViewMode
  })

  // Watch for changes and persist to store
  watch([showConflictsOnly, viewMode, conflictDiffViewMode], async () => {
    await appStore.setConflictViewerSettings({
      showConflictsOnly: showConflictsOnly.value,
      viewMode: viewMode.value,
      conflictDiffViewMode: conflictDiffViewMode.value
    })
  })

  return {
    showConflictsOnly,
    viewMode,
    conflictDiffViewMode
  }
}