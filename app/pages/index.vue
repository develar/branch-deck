<template>
  <UContainer class="py-4">
    <BranchCreator/>
  </UContainer>
</template>

<script lang="ts" setup>
import { onMounted } from "vue"
import { useAutoUpdate } from "~/composables/autoUpdate"

const { checkForUpdatesOnStartup } = useAutoUpdate()

onMounted(() => {
  checkForUpdatesOnStartup()
})
</script>
