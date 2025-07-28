<template>
  <UApp>
    <NuxtPage />
  </UApp>
</template>

<script lang="ts" setup>
// Provide model state for AI features
provideModelState()

// Initialize color selector to listen for menu events
useColorSelector()

// Initialize store handlers for main window
initializeStoreHandlers()
</script>
