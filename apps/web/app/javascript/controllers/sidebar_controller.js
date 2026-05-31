import { Controller } from "@hotwired/stimulus"

// Slides the left nav in/out on small screens. Mounted on <body> so the topbar
// menu button and the <nav> panel are both in scope.
export default class extends Controller {
  static targets = ["panel"]

  toggle() {
    if (this.hasPanelTarget) this.panelTarget.classList.toggle("is-open")
  }
}
