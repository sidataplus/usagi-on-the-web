import { Controller } from "@hotwired/stimulus"

// Manages row-checkbox selection in the mapping table: a select-all box, a
// floating bulk-actions toolbar that appears when ≥1 row is selected, and
// syncing the selected ids into the bulk-action form's hidden field.
export default class extends Controller {
  static targets = ["checkbox", "selectAll", "toolbar", "count", "ids"]

  connect() {
    this.refresh()
  }

  toggleAll() {
    const checked = this.selectAllTarget.checked
    this.checkboxTargets.forEach((box) => { box.checked = checked })
    this.refresh()
  }

  refresh() {
    const selected = this.checkboxTargets.filter((box) => box.checked)
    const ids = selected.map((box) => box.value)

    if (this.hasToolbarTarget) {
      // The toolbar hides via the `hidden` attribute (`.bulkbar[hidden]`), so
      // toggle the attribute itself rather than a class.
      this.toolbarTarget.hidden = ids.length === 0
    }
    if (this.hasCountTarget) {
      this.countTarget.textContent = ids.length
    }
    if (this.hasIdsTarget) {
      this.idsTarget.value = ids.join(",")
    }
    if (this.hasSelectAllTarget) {
      this.selectAllTarget.checked = ids.length > 0 && ids.length === this.checkboxTargets.length
      this.selectAllTarget.indeterminate = ids.length > 0 && ids.length < this.checkboxTargets.length
    }
  }

  clear() {
    this.checkboxTargets.forEach((box) => { box.checked = false })
    this.refresh()
  }
}
