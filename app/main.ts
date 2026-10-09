import { createApp } from "vue"
import ui from "@nuxt/ui/vue-plugin"
import App from "./app.vue"
import { router } from "./router"
import "./assets/css/main.css"

createApp(App).use(router).use(ui).mount("#app")
