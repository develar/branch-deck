<template>
  <UContainer class="pb-4">
    <BranchCreator />
  </UContainer>
</template>

<script lang="ts" setup>
const { checkForUpdatesOnStartup } = useAutoUpdate()

onMounted(() => {
  checkForUpdatesOnStartup()
})
</script>
