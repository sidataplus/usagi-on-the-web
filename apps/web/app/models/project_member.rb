class ProjectMember < ApplicationRecord
  include HasPrefixedId

  prefixed_id "pmem"

  ROLES = %w[owner admin reviewer observer].freeze

  belongs_to :project
  belongs_to :user

  validates :role, inclusion: { in: ROLES }
  validates :user_id, uniqueness: { scope: :project_id }

  def can_import?
    owner? || admin?
  end

  def can_run_suggestions?
    owner? || admin? || reviewer?
  end

  def can_review?
    owner? || admin? || reviewer?
  end

  def can_export?
    owner? || admin? || reviewer?
  end

  def can_manage_members?
    owner? || admin?
  end

  def owner?
    role == "owner"
  end

  def admin?
    role == "admin"
  end

  def reviewer?
    role == "reviewer"
  end

  def observer?
    role == "observer"
  end

  def role_label
    role.to_s.humanize
  end
end
