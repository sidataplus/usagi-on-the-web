class SettingsController < ApplicationController
  def show
    @prefs = current_user.prefs
  end

  # Known preference keys; keeps mass-assignment explicit (no permit!).
  PREFERENCE_KEYS = %i[rows_per_page email_notifications assignment_alerts weekly_digest].freeze

  def update
    submitted = params.fetch(:preferences, {})
    submitted = submitted.permit(*PREFERENCE_KEYS).to_h if submitted.respond_to?(:permit)
    current_user.update(preferences: current_user.prefs.merge(submitted))
    redirect_to settings_path, notice: "Preferences saved."
  end
end
