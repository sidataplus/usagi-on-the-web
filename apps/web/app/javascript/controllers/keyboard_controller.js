import { Controller } from "@hotwired/stimulus"

// Global review keyboard shortcuts. Mounted on <body>. Shortcuts only fire when
// an actionable mapping context is present (a focused row or an open modal) and
// never while typing in a form field. Each shortcut clicks the matching button
// so the behavior stays in the markup, not duplicated here.
export default class extends Controller {
  connect() {
    this.boundKey = this.handle.bind(this)
    document.addEventListener("keydown", this.boundKey)
  }

  disconnect() {
    document.removeEventListener("keydown", this.boundKey)
  }

  handle(event) {
    if (event.metaKey || event.ctrlKey || event.altKey) return

    const tag = (event.target.tagName || "").toUpperCase()
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || event.target.isContentEditable) return

    const scope = document.querySelector("[data-keyboard-scope]")
    if (!scope) return

    const map = {
      a: "approve",
      f: "flag",
      i: "invalid",
      n: "next",
      e: "edit"
    }
    const arrows = {
      ArrowLeft: "prev",
      ArrowRight: "next-record"
    }

    const action = map[event.key.toLowerCase()] || arrows[event.key]
    if (!action) return

    const button = scope.querySelector(`[data-shortcut="${action}"]`)
    if (button) {
      event.preventDefault()
      button.click()
    }
  }
}
