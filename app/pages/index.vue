<template>
  <UContainer class="py-4">
    <BranchCreator/>
  </UContainer>
</template>

<script lang="ts" setup>
import { onMounted } from "vue"
import { useAutoUpdate } from "~/composables/git/autoUpdate"

const { checkForUpdatesOnStartup } = useAutoUpdate()

onMounted(() => {
  checkForUpdatesOnStartup()
})
</script>
