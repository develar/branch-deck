<template>
  <UApp>
    <NuxtPage />
  </UApp>
</template>

<script lang="ts" setup>
import { provide, onMounted } from "vue"
import { appStore, appStoreKey, initializeStoreHandlers } from "~/utils/app-store"
import { useColorSelector } from "./composables/colorSelector"

// Provide app store for injection in child components
provide(appStoreKey, appStore)

// Initialize color selector to listen for menu events after mount
useColorSelector(appStore)

// Initialize store handlers for main window
onMounted(() => {
  initializeStoreHandlers()
})
</script>
