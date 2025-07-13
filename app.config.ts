export default defineAppConfig({
  theme: {
    radius: 0.25
  },
  ui: {
    colors: {
      primary: "green",
    }
  },
  toast: {
    slots: {
      // make text selectable
      description: "text-sm text-muted select-text",
    },
  },
})