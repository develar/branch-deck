<template>
  <UModal
    :overlay="false"
    title="Theme"
  >
    <template #body>
      <div class="w-72 px-6 py-4 flex flex-col gap-4">
        <!-- Primary Color Selection -->
        <fieldset>
          <legend class="text-[11px] leading-none font-semibold mb-2">
            Primary
          </legend>
          <div class="grid grid-cols-3 gap-1 -mx-2">
            <!-- Color Options -->
            <UButton
              v-for="color in primaryColorOptions"
              :key="color.name"
              variant="outline"
              color="neutral"
              size="sm"
              :class="[
                'capitalize ring-default rounded-sm text-[11px]',
                props.currentColor === color.name ? 'bg-elevated' : 'hover:bg-elevated/50'
              ]"
              @click="selectColor(color.name)"
            >
              <template #leading>
                <span
                  class="inline-block size-2 rounded-full"
                  :style="{ backgroundColor: color.hex }"
                />
              </template>
              {{ color.name }}
            </UButton>
          </div>
        </fieldset>

        <!-- Neutral Color Selection -->
        <!-- <fieldset>
          <legend class="text-[11px] leading-none font-semibold mb-2">
            Neutral
          </legend>
          <div class="grid grid-cols-3 gap-1 -mx-2">
            <UButton
              v-for="color in neutralColorOptions"
              :key="color.name"
              variant="outline"
              color="neutral"
              size="sm"
              :class="[
                'capitalize ring-default rounded-sm text-[11px]',
                props.currentNeutral === color.name ? 'bg-elevated' : 'hover:bg-elevated/50'
              ]"
              @click="selectNeutral(color.name)"
            >
              <template #leading>
                <span
                  class="inline-block size-2 rounded-full"
                  :style="{ backgroundColor: color.hex }"
                />
              </template>
              {{ color.name }}
            </UButton>
          </div>
        </fieldset> -->

        <!-- Radius Selection -->
        <!-- <fieldset>
          <legend class="text-[11px] leading-none font-semibold mb-2">
            Radius
          </legend>
          <div class="grid grid-cols-5 gap-1 -mx-2">
            <UButton
              v-for="radius in radiuses"
              :key="radius"
              variant="outline"
              color="neutral"
              size="sm"
              :class="[
                'justify-center px-0 ring-default rounded-sm text-[11px]',
                props.currentRadius === radius ? 'bg-elevated' : 'hover:bg-elevated/50'
              ]"
              @click="selectRadius(radius)"
            >
              {{ String(radius) }}
            </UButton>
          </div>
        </fieldset> -->
      </div>
    </template>
  </UModal>
</template>

<script lang="ts" setup>
import colors from "tailwindcss/colors"
import { omit } from "#ui/utils"

interface Props {
  currentColor?: string
  currentRadius?: number
  currentNeutral?: string
  onChange?: (setting: { type: string, value: string }) => void
}

const props = withDefaults(defineProps<Props>(), {
  currentColor: "blue",
  currentRadius: 0.25,
  currentNeutral: "slate",
  onChange: () => {},
})

const emit = defineEmits<{
  change: [value: { type: string, value: string }]
}>()

// Compute colors exactly like ThemePicker
const neutralColors = ["slate", "gray", "zinc", "neutral", "stone"]
const colorsToOmit = ["inherit", "current", "transparent", "black", "white", ...neutralColors]
const primaryColors = Object.keys(omit(colors, colorsToOmit as (keyof typeof colors)[]))

// Create color objects with hex values from Tailwind colors
const primaryColorOptions = computed(() => {
  return primaryColors.map(color => ({
    name: color,
    hex: colors[color as keyof typeof colors]?.[500] || "#3b82f6", // fallback to blue
  }))
})

// const neutralColorOptions = computed(() => {
//   return neutralColors.map(color => ({
//     name: color,
//     hex: colors[color as keyof typeof colors]?.[500] || '#64748b' // fallback to slate
//   }))
// })

// Radius options
// const radiuses = [0, 0.125, 0.25, 0.375, 0.5]

const selectColor = (color: string) => {
  const setting = { type: "primary", value: color }
  emit("change", setting)
  props.onChange?.(setting)
}

// const selectNeutral = (color: string) => {
//   const setting = { type: 'neutral', value: color }
//   emit('change', setting)
//   props.onChange?.(setting)
// }

// const selectRadius = (radius: number) => {
//   const setting = { type: 'radius', value: String(radius) }
//   emit('change', setting)
//   props.onChange?.(setting)
// }
</script>
