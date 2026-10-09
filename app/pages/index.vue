<template>
  <UContainer class="pb-4" data-testid="app-container">
    <BranchCreator />
  </UContainer>
</template>

<script lang="ts" setup>
import { onMounted } from "vue"
import BranchCreator from "~/components/BranchCreator.vue"
import { useAutoUpdate } from "~/composables/autoUpdate"

const { checkForUpdatesOnStartup } = useAutoUpdate()

onMounted(() => {
  checkForUpdatesOnStartup()
})
</script>
