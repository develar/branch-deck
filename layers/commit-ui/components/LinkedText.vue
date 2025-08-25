<template>
  <span :class="textClass">
    <template v-if="segments.length === 1 && segments[0]?.type === 'text'">
      {{ segments[0]?.content }}
    </template>
    <template v-else>
      <template v-for="(segment, index) in segments" :key="index">
        <ULink
          v-if="segment.type === 'link'"
          :external="true"
          target="_blank"
          :to="segment.href"
        >
          {{ segment.content }}
        </ULink>
        <span v-else>{{ segment.content }}</span>
      </template>
    </template>
  </span>
</template>

<script lang="ts" setup>
interface Props {
  text: string
  textClass?: string
}

const props = withDefaults(defineProps<Props>(), {
  textClass: "",
})

const { parseTextSegments } = useIssueLinks()

// Parse the text into segments
const segments = computed(() => {
  return parseTextSegments(props.text)
})
</script>
