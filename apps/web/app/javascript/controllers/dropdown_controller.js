import { Controller } from "@hotwired/stimulus"

// Generic click-to-open menu with click-outside + Escape to close.
export default class extends Controller {
  static targets = ["menu"]

  connect() {
    this.boundAway = this.closeOnClickAway.bind(this)
    this.boundKey = this.closeOnEscape.bind(this)
  }

  toggle(event) {
    event.stopPropagation()
    const willOpen = this.menuTarget.classList.contains("hidden")
    this.menuTarget.classList.toggle("hidden")
    if (willOpen) {
      document.addEventListener("click", this.boundAway)
      document.addEventListener("keydown", this.boundKey)
    } else {
      this.removeListeners()
    }
  }

  close() {
    this.menuTarget.classList.add("hidden")
    this.removeListeners()
  }

  closeOnClickAway(event) {
    if (!this.element.contains(event.target)) this.close()
  }

  closeOnEscape(event) {
    if (event.key === "Escape") this.close()
  }

  removeListeners() {
    document.removeEventListener("click", this.boundAway)
    document.removeEventListener("keydown", this.boundKey)
  }

  disconnect() {
    this.removeListeners()
  }
}
