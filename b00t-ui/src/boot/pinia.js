// Pinia boot file — registers Pinia with the Quasar app
// https://pinia.vuejs.org/

import { createPinia } from 'pinia'

export default ({ app }) => {
  app.use(createPinia())
}
