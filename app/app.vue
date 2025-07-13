<template>
  <UApp>
    <NuxtPage />
  </UApp>
</template>

<script lang="ts" setup>
import { provide, computed, onMounted } from 'vue'
import { appStore, appStoreKey, initializeStoreHandlers } from '~/utils/app-store'
import { useColorSelector } from './composables/colorSelector'

const appConfig = useAppConfig()

// CSS custom properties injection for radius
const radius = computed(() => `:root { --ui-radius: ${appConfig.theme?.radius || 0.25}rem; }`)

useHead({
  style: [
    { innerHTML: radius, id: 'nuxt-ui-radius', tagPriority: -2 }
  ]
})

// Provide app store for injection in child components
provide(appStoreKey, appStore)

// Initialize color selector to listen for menu events after mount
useColorSelector(appStore)

// Initialize store handlers for main window
onMounted(() => {
  initializeStoreHandlers()
})
</script>
