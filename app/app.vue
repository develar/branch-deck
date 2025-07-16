<template>
  <UApp>
    <NuxtPage />
  </UApp>
</template>

<script lang="ts" setup>
import { provide, onMounted } from "vue"
import { appStore, appStoreKey, initializeStoreHandlers } from "~/utils/app-store"
import { useModelDownload } from "./composables/ai/useModelDownload"

// Provide app store for injection in child components
provide(appStoreKey, appStore)

// Initialize color selector to listen for menu events after mount
useColorSelector(appStore)

// Initialize model download handler
useModelDownload()

// Initialize store handlers for main window
onMounted(() => {
  initializeStoreHandlers()
})
</script>
