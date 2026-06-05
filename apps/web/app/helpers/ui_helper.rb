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

  JOB_STATE_TONE = {
    "queued" => "neutral",
    "running" => "accent",
    "succeeded" => "ok",
    "succeeded_with_errors" => "warn",
    "failed" => "bad",
    "cancelled" => "neutral"
  }.freeze

  # Tone for an engine/export job state badge.
  def job_state_tone(state)
    JOB_STATE_TONE.fetch(state.to_s, "neutral")
  end

  def status_badge(mapping)
    tone = STATUS_TONE.fetch(mapping.status, "neutral")
    dot_badge(mapping.status_label, tone)
  end

  def project_status_badge(project)
    tone = PROJECT_TONE.fetch(project.status, "neutral")
    dot_badge(project.status_label, tone)
  end

  # Confidence band (high/medium/low) for a 0..1 match score.
  def score_tone(score)
    return "neutral" if score.nil?

    score >= 0.8 ? "ok" : (score >= 0.5 ? "warn" : "bad")
  end

  # Match score as a percentage chip, coloured by confidence band.
  def score_badge(score)
    return tag.span("—", class: "subtle small") if score.nil?

    tag.span "#{(score * 100).round}%", class: "badge badge--#{score_tone(score)}"
  end

  # The "Bar" readout: a number paired with a confidence meter that fills to the
  # score and is tinted by band. The single visualisation used everywhere a
  # match score appears (review table, cockpit candidate table, command lane).
  def score_bar(score)
    return tag.span("—", class: "subtle small") if score.nil?

    pct = (score.to_f * 100).round.clamp(0, 100)
    tag.div class: "scorebar scorebar--#{score_tone(score)}", role: "img", aria: { label: "Match score #{pct}%" } do
      safe_join([
        tag.span(safe_join([ pct.to_s, tag.span("%", class: "scorebar__pct") ]), class: "scorebar__num"),
        tag.span(tag.span("", class: "scorebar__fill", style: "inline-size: #{pct}%"), class: "scorebar__track")
      ])
    end
  end

  # A status pill with a leading semaphore dot (currentColor).
  def dot_badge(label, tone)
    tag.span class: "badge badge--#{tone}" do
      safe_join([ tag.span("", class: "badge__dot", aria: { hidden: "true" }), label ])
    end
  end

  # Active state for a nav link.
  def nav_link_class(active)
    active ? "nav__link is-active" : "nav__link"
  end

  # [label, value] role options a manager can assign (every role except owner).
  def assignable_roles
    ProjectMember::ROLES.reject { |role| role == "owner" }.map { |role| [ role.humanize, role ] }
  end

  # Review stats for one project drawn from a preloaded grouped-count hash
  # ({ [project_id, "APPROVED"] => n, ... }), avoiding per-card COUNT queries.
  def project_review_stats(counts, project_id)
    approved = counts.fetch([ project_id, "APPROVED" ], 0)
    total = counts.select { |(pid, _status), _n| pid == project_id }.values.sum
    {
      total: total,
      approved: approved,
      flagged: counts.fetch([ project_id, "FLAGGED" ], 0),
      unchecked: counts.fetch([ project_id, "UNCHECKED" ], 0),
      completion: total.zero? ? 0 : ((approved.to_f / total) * 100).round
    }
  end
end
