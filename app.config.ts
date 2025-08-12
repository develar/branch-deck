export default defineAppConfig({
  ui: {
    colors: {
      primary: "green",
    },
  },
  toast: {
    slots: {
      // make text selectable
      description: "text-sm text-muted select-text",
    },
  },
})
