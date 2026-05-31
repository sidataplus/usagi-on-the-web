import { Controller } from "@hotwired/stimulus"

// Mounted on the mapping detail modal content rendered inside the top-level
// #modal turbo-frame. Closing empties the frame so it can be reopened.
export default class extends Controller {
  connect() {
    this.boundKey = this.closeOnEscape.bind(this)
    document.addEventListener("keydown", this.boundKey)
    document.body.classList.add("overflow-hidden")
  }

  disconnect() {
    document.removeEventListener("keydown", this.boundKey)
    document.body.classList.remove("overflow-hidden")
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

  closeOnEscape(event) {
    if (event.key === "Escape") this.close()
  }
}
