<template>
  <UApp>
    <NuxtPage />
  </UApp>
</template>

<script lang="ts" setup>
// Initialize color selector to listen for menu events
useColorSelector()

// Initialize store handlers for main window
initializeStoreHandlers()
</script>
