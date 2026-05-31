module UiHelper
  # Helpers that encapsulate status → CSS-class logic. Plain structure lives in
  # the templates; only the meaningful mappings are kept here.

  STATUS_TONE = {
    "unchecked" => "neutral",
    "approved" => "ok",
    "flagged" => "warn",
    "invalid_status" => "bad"
  }.freeze

  PROJECT_TONE = {
    "active" => "accent",
    "completed" => "ok",
    "archived" => "neutral"
  }.freeze

  def status_badge(mapping)
    tone = STATUS_TONE.fetch(mapping.status, "neutral")
    tag.span mapping.status_label, class: "badge badge--#{tone}"
  end

  def project_status_badge(project)
    tone = PROJECT_TONE.fetch(project.status, "neutral")
    tag.span project.status_label, class: "badge badge--#{tone}"
  end

  # Match score as a percentage chip, coloured by confidence band.
  def score_badge(score)
    return tag.span("—", class: "subtle small") if score.nil?

    tone = if score >= 0.8 then "ok" elsif score >= 0.5 then "warn" else "bad" end
    tag.span "#{(score * 100).round}%", class: "badge badge--#{tone}"
  end

  # Active state for a nav link.
  def nav_link_class(active)
    active ? "nav__link is-active" : "nav__link"
  end
end
