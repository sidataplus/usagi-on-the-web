import { Controller } from "@hotwired/stimulus"

// Simple expand/collapse for the sidebar beta-info box and similar sections.
export default class extends Controller {
  static targets = ["content", "icon"]

  toggle() {
    this.contentTarget.classList.toggle("hidden")
    if (this.hasIconTarget) {
      this.iconTarget.classList.toggle("rotate-180")
    }
  }
}
