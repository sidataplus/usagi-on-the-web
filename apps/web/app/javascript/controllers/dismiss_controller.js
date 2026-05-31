import { Controller } from "@hotwired/stimulus"

// Dismisses a flash message; auto-hides after a short delay.
export default class extends Controller {
  connect() {
    this.timeout = setTimeout(() => this.close(), 6000)
  }

  close() {
    this.element.remove()
  }

  disconnect() {
    if (this.timeout) clearTimeout(this.timeout)
  }
}
