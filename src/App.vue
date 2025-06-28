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
import BranchCreator from "./components/BranchCreator.vue"
</script>
