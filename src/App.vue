<template>
  <UApp>
    <UContainer class="py-4">
      <Suspense>
        <BranchCreator />
      </Suspense>
    </UContainer>
  </UApp>
</template>

<script lang="ts" setup>
import { onMounted } from "vue"
import BranchCreator from "./components/BranchCreator.vue"
import { useAutoUpdate } from "./composables/autoUpdate"

const { checkForUpdatesOnStartup } = useAutoUpdate()

onMounted(() => {
  checkForUpdatesOnStartup()
})
</script>
