<template>
  <UCard>
    <template #header>
      <h2>Branch Creator</h2>
      <!--<p class="text-xs">-->
      <!--  Create and manage virtual branches from commit prefixes-->
      <!--</p>-->
    </template>
    <div class="space-y-6">
      <!-- Repository Path -->
      <UFormField label="Repository Path" name="repo-path">
        <div class="flex gap-3">
          <UInput
            v-model="repositoryPath"
            placeholder="Enter repository path..."
            class="flex-1"
            :disabled="isProcessing"
          />
          <UButton
            icon="i-heroicons-folder-open"
            variant="outline"
            @click="browseRepository"
            :disabled="isProcessing"
          >
            Browse
          </UButton>
        </div>
      </UFormField>

      <!-- Branch Prefix -->
      <UFormField label="Branch Prefix" name="branch-prefix">
        <UInput
          v-model="branchPrefix"
          placeholder="Enter branch prefix..."
          class="flex-1"
          :disabled="isProcessing"
          @keyup.enter="createBranches"
        />
      </UFormField>

      <!-- Actions -->
      <div class="flex flex-col sm:flex-row gap-3">
        <UButton
          size="lg"
          :loading="isProcessing"
          :disabled="!repositoryPath"
          @click="createBranches"
        >
          <template #leading>
            <span v-if="!isProcessing">🌿</span>
          </template>
          {{ isProcessing ? 'Processing...' : 'Create Virtual Branches' }}
        </UButton>
      </div>
    </div>
    <!-- Loading State -->
    <div v-if="isProcessing" class="text-center py-8">
      <div class="inline-block w-8 h-8 border-4 border-blue-200 border-t-blue-600 rounded-full animate-spin mb-4"></div>
      <p class="text-gray-600">Processing repository...</p>
    </div>

    <!-- Results -->
    <UCard v-if="result && !isProcessing" class="mt-6">
      <UAlert
        v-if="result.error"
        icon="i-heroicons-x-circle"
        color="red"
        variant="soft"
        :title="`Error: ${result.error}`"
      />

      <div v-else class="space-y-4">
        <UAlert
          icon="i-heroicons-check-circle"
          color="green"
          variant="soft"
          :title="`${result.message}`"
        />

        <!-- Branches List -->
        <div v-if="result.branches && result.branches.length > 0" class="space-y-4">
          <UCard
            v-for="branch in result.branches"
            :key="branch.name"
            :class="['relative', { 'border-l-4 border-l-red-500': branch.error, 'border-l-4 border-l-green-500': !branch.error }]"
          >
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4">
              <h4 class="text-lg font-semibold flex items-center gap-2">
                🌿 {{ branch.name }}
              </h4>
              <UBadge
                v-if="!branch.error"
                :color="branch.action?.toLowerCase() === 'created' ? 'green' : 'blue'"
                variant="soft"
              >
                {{ branch.action }}
              </UBadge>
            </div>

            <UAlert
              v-if="branch.error"
              icon="i-heroicons-exclamation-triangle"
              color="red"
              variant="soft"
              :title="`Error: ${branch.error}`"
            />

            <div v-else class="space-y-3">
              <p class="text-sm text-gray-600">
                <strong>{{ branch.commitCount }} commit{{ branch.commitCount !== 1 ? 's' : '' }}</strong>
              </p>

              <!-- Commit List -->
              <div v-if="branch.commitDetails && branch.commitDetails.length > 0" class="space-y-2">
                <div
                  v-for="commit in branch.commitDetails"
                  :key="commit.hash"
                  class="flex items-center gap-3 p-3 bg-gray-50 rounded-lg"
                >
                  <span class="text-lg">{{ commit.isNew ? '✨' : '🔄' }}</span>
                  <span class="commit-hash">{{ commit.hash.substring(0, 8) }}</span>
                  <span class="flex-1 text-sm text-gray-700">{{ commit.message }}</span>
                </div>
              </div>
            </div>
          </UCard>
        </div>
      </div>
    </UCard>
  </UCard>
</template>

<script>
import {CreateVirtualBranches, GetRepositoryInfo, OpenDirectoryDialog} from '../../wailsjs/go/main/App.js'

export default {
  name: 'BranchCreator',
  data() {
    return {
      repositoryPath: '',
      branchPrefix: 'develar/',
      isProcessing: false,
      result: null,
      repositoryInfo: null
    }
  },
  methods: {
    async createBranches() {
      if (!this.repositoryPath.trim()) {
        this.showError('Please enter a repository path')
        return
      }

      this.isProcessing = true
      this.result = null
      this.repositoryInfo = null

      try {
        const result = await CreateVirtualBranches(this.repositoryPath.trim(), this.branchPrefix.trim())
        this.result = result
      } catch (error) {
        this.result = {
          success: false,
          error: `Failed to process repository: ${error.message || error}`
        }
      } finally {
        this.isProcessing = false
      }
    },

    async getRepositoryInfo() {
      if (!this.repositoryPath.trim()) {
        this.showError('Please enter a repository path')
        return
      }

      this.isProcessing = true
      this.result = null

      try {
        const info = await GetRepositoryInfo(this.repositoryPath.trim())
        this.repositoryInfo = info
      } catch (error) {
        this.repositoryInfo = {
          error: `Failed to get repository info: ${error.message || error}`
        }
      } finally {
        this.isProcessing = false
      }
    },

    async browseRepository() {
      try {
        const path = await OpenDirectoryDialog()
        if (path) {
          this.repositoryPath = path
        }
      } catch (error) {
        console.error('Failed to open directory dialog:', error)
      }
    },

    showError(message) {
      this.result = {
        success: false,
        error: message
      }
    }
  }
}
</script>