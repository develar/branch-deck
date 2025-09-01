<template>
  <UContainer class="pb-4" data-testid="app-container">
    <BranchCreator />
  </UContainer>
</template>

<script lang="ts" setup>
const { checkForUpdatesOnStartup } = useAutoUpdate()

onMounted(() => {
  checkForUpdatesOnStartup()
})
</script>
