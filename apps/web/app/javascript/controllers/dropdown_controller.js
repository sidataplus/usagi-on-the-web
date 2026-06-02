import { Controller } from "@hotwired/stimulus"

// Click-to-open menu with click-outside + Escape to close. The panel is hidden
// with the `hidden` attribute (see `.menu__panel[hidden]` in components.css), so
// we toggle the attribute itself — not a CSS class — and keep aria-expanded in
// sync on the trigger button.
export default class extends Controller {
  static targets = ["menu"]

  connect() {
    this.boundAway = this.closeOnClickAway.bind(this)
    this.boundKey = this.closeOnEscape.bind(this)
    this.trigger?.setAttribute("aria-expanded", this.menuTarget.hidden ? "false" : "true")
  }

  toggle(event) {
    event.stopPropagation()
    if (this.menuTarget.hidden) this.open()
    else this.close()
  }

  open() {
    this.menuTarget.hidden = false
    this.trigger?.setAttribute("aria-expanded", "true")
    document.addEventListener("click", this.boundAway)
    document.addEventListener("keydown", this.boundKey)
  }

  close() {
    if (this.menuTarget.hidden) return
    this.menuTarget.hidden = true
    this.trigger?.setAttribute("aria-expanded", "false")
    this.removeListeners()
  }

  closeOnClickAway(event) {
    if (!this.element.contains(event.target)) this.close()
  }

  closeOnEscape(event) {
    if (event.key !== "Escape") return
    this.close()
    this.trigger?.focus()
  }

  removeListeners() {
    document.removeEventListener("click", this.boundAway)
    document.removeEventListener("keydown", this.boundKey)
  }

  get trigger() {
    return this.element.querySelector('[data-action~="dropdown#toggle"]')
  }

  disconnect() {
    this.removeListeners()
  }
}
