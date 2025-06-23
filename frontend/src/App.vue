<template>
  <UContainer class="py-4">
    <!-- Header -->
    <!--<UCard class="gradient-header text-center mb-6" variant="subtle">-->
    <!--  <div class="py-8">-->
    <!--    <h1>🌿 Virtual Branch Manager</h1>-->
    <!--    <p class="text-sm">-->
    <!--      Create and manage virtual branches from commit prefixes-->
    <!--    </p>-->
    <!--  </div>-->
    <!--</UCard>-->

    <!-- Main Content -->
    <BranchCreator/>
  </UContainer>
</template>

<script>
import BranchCreator from './components/BranchCreator.vue'

export default {
  name: 'App',
  components: {
    BranchCreator
  }
}
</script>
