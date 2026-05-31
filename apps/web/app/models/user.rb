class User < ApplicationRecord
  include HasPrefixedId

  prefixed_id "usr"

  has_secure_password validations: false

  has_many :project_members, dependent: :destroy
  has_many :projects, through: :project_members
  has_many :created_projects, class_name: "Project", foreign_key: :created_by_id,
                              inverse_of: :created_by, dependent: :restrict_with_error
  has_many :import_sessions, foreign_key: :created_by_id, dependent: :restrict_with_error
  has_many :requested_exports, class_name: "Export", foreign_key: :requested_by_id,
                               inverse_of: :requested_by, dependent: :nullify
  has_many :audit_events, dependent: :nullify
  has_many :comments, dependent: :nullify

  before_validation :normalize_email

  validates :email, presence: true, uniqueness: { case_sensitive: false }
  validates :name, presence: true
  validates :password, length: { minimum: 8 }, allow_nil: true

  def can_access_admin?
    admin?
  end

  def display_name
    name.presence || email.to_s.split("@").first
  end

  def initials
    display_name.to_s.split(/[\s_]+/).map { |part| part[0] }.first(2).join.upcase
  end

  def prefs
    preferences || {}
  end

  def role
    admin? ? "admin" : "user"
  end

  def role=(value)
    self.admin = value.to_s.in?(%w[admin superadmin])
  end

  def self.roles
    { "admin" => true, "user" => false }
  end

  def last_login_at
    last_seen_at
  end

  def last_login_at=(value)
    self.last_seen_at = value
  end

  def avatar_url
    nil
  end

  def avatar_url=(_value)
  end

  def role_label
    role.humanize
  end

  private
    def normalize_email
      self.email = email.to_s.strip.downcase
    end
end
