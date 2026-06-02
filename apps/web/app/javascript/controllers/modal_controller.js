import { Controller } from "@hotwired/stimulus"

// Mounted on the mapping detail / export modal rendered inside the top-level
// #modal turbo-frame. Locks background scroll, closes on Escape or backdrop
// click, moves focus into the dialog, traps Tab within it, and restores focus
// to the opener on close. Closing empties the frame so it can be reopened.
export default class extends Controller {
  connect() {
    this.dialog = this.element.querySelector("[role=dialog]") || this.element
    // Stash the opener on the persistent #modal frame so it survives advances
    // (each advance replaces this controller's element). Capture once: on an
    // advance, activeElement is already <body>, so we must not overwrite it.
    const frame = document.getElementById("modal")
    if (frame && !frame._modalOpener) {
      const opener = document.activeElement
      if (opener && opener !== document.body && opener.focus && !this.element.contains(opener)) {
        frame._modalOpener = opener
      }
    }
    this.boundKey = this.onKeydown.bind(this)
    document.addEventListener("keydown", this.boundKey)
    document.body.classList.add("overflow-hidden")
    requestAnimationFrame(() => this.focusDialog())
  }

  disconnect() {
    document.removeEventListener("keydown", this.boundKey)
    // Only a true close (the frame was emptied) unlocks scroll and restores
    // focus; an advance keeps the next cockpit in the frame.
    const frame = document.getElementById("modal")
    if (frame && frame.innerHTML.trim() !== "") return

    document.body.classList.remove("overflow-hidden")
    const opener = frame && frame._modalOpener
    if (opener && opener.isConnected && opener.focus) opener.focus()
    if (frame) delete frame._modalOpener
  }

  // Close when the backdrop (the element carrying the action) is clicked,
  // but not when a click bubbles up from the dialog itself.
  backdrop(event) {
    if (event.target === event.currentTarget) this.close()
  }

  close() {
    const frame = document.getElementById("modal")
    if (frame) frame.innerHTML = ""
  }

  onKeydown(event) {
    if (event.key === "Escape") {
      this.close()
    } else if (event.key === "Tab") {
      this.trapFocus(event)
    }
  }

  focusDialog() {
    if (!this.dialog.hasAttribute("tabindex")) this.dialog.setAttribute("tabindex", "-1")
    this.dialog.focus()
  }

  focusables() {
    return Array.from(
      this.dialog.querySelectorAll(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
      )
    ).filter((el) => el.offsetParent !== null)
  }

  trapFocus(event) {
    const items = this.focusables()
    if (items.length === 0) return

    const first = items[0]
    const last = items[items.length - 1]
    const active = document.activeElement

    if (event.shiftKey && (active === first || active === this.dialog)) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && active === last) {
      event.preventDefault()
      first.focus()
    }
  }
}
