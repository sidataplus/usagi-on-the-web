# frozen_string_literal: true

class LocalDeployAuth
  DEFAULT_EMAIL = "local@usagi.test"
  DEFAULT_NAME = "Local Reviewer"

  def self.enabled?(env = ENV)
    env["USAGI_LOCAL_DEPLOY"] == "1"
  end

  def self.user_email(env = ENV)
    env.fetch("USAGI_LOCAL_DEPLOY_USER_EMAIL", DEFAULT_EMAIL).to_s.strip.downcase
  end

  def self.ensure_user!
    return unless enabled?

    user = User.find_or_initialize_by(email: user_email)
    user.name = DEFAULT_NAME
    user.admin = true
    user.password = SecureRandom.hex(32) if user.new_record? || user.password_digest.blank?
    user.save!
    user
  end

  def self.sign_in!(session)
    return unless enabled?

    user = ensure_user!
    session[:user_id] = user.id
    user.update!(last_seen_at: Time.current)
    user
  end
end
