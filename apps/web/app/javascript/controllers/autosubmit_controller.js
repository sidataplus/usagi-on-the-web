import { Controller } from "@hotwired/stimulus"

// Debounced auto-submit for filter/search forms. The form targets a Turbo
// Frame, so submitting re-renders only that frame.
export default class extends Controller {
  static values = { delay: { type: Number, default: 300 } }

  submit() {
    clearTimeout(this.timeout)
    this.timeout = setTimeout(() => {
      this.element.requestSubmit()
    }, this.delayValue)
  }

  disconnect() {
    clearTimeout(this.timeout)
  }
}
