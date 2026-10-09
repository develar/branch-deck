<template>
  <UApp>
    <RouterView />
  </UApp>
</template>

<script lang="ts" setup>
import { RouterView } from "vue-router"
import UApp from "@nuxt/ui/components/App.vue"
import { useColorSelector } from "~/composables/colorSelector"
import { initializeStoreHandlers } from "@layers/core/utils/app-store"

// Initialize color selector to listen for menu events
useColorSelector()

// Initialize store handlers for main window
initializeStoreHandlers()
</script>
