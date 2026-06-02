class Project < ApplicationRecord
  include HasPrefixedId

  prefixed_id "proj"

  belongs_to :created_by, class_name: "User", inverse_of: :created_projects
  has_many :project_members, dependent: :destroy
  has_many :project_memberships, class_name: "ProjectMember", dependent: :destroy
  has_many :members, through: :project_members, source: :user
  has_many :import_sessions, dependent: :destroy
  has_many :import_batches, class_name: "ImportSession", dependent: :destroy
  has_many :source_terms, dependent: :destroy
  has_many :mappings, dependent: :destroy
  has_many :mapping_candidates, dependent: :destroy
  has_many :engine_jobs, dependent: :destroy
  has_many :exports, dependent: :destroy
  has_many :mapping_exports, class_name: "Export", dependent: :destroy
  has_many :audit_events, dependent: :destroy
  has_many :comments, dependent: :destroy

  STATUSES = %w[active archived completed].freeze
  MAPPING_DOMAINS = %w[Drug Condition Procedure Measurement Observation Device Mixed].freeze

  validates :name, presence: true
  validates :status, inclusion: { in: STATUSES }
  validates :mapping_domain, inclusion: { in: MAPPING_DOMAINS }

  after_create :ensure_owner_member

  def self.statuses
    STATUSES.index_with(&:itself)
  end

  def owner
    created_by
  end

  def owner=(user)
    self.created_by = user
  end

  def target_vocabularies
    target_vocabulary_ids || []
  end

  def target_vocabularies=(values)
    list = values.is_a?(String) ? values.split(",") : Array(values)
    self.target_vocabulary_ids = list.map { |value| value.to_s.strip }.reject(&:blank?)
  end

  def drug_domain?
    mapping_domain == "Drug"
  end

  def hybrid_search_domain?
    !drug_domain?
  end

  def active?
    status == "active"
  end

  def archived?
    status == "archived"
  end

  def completed?
    status == "completed"
  end

  def total_mappings
    mappings.count
  end

  def approved_count
    mappings.approved.count
  end

  def flagged_count
    mappings.flagged.count
  end

  def unchecked_count
    mappings.unchecked.count
  end

  def invalid_count
    mappings.invalid_status.count
  end

  def reviewed_count
    total_mappings - unchecked_count
  end

  def completion_percentage
    total = total_mappings
    return 0 if total.zero?

    ((approved_count.to_f / total) * 100).round
  end

  def status_label
    status.to_s.humanize
  end

  private
    def ensure_owner_member
      project_members.find_or_create_by!(user: created_by) { |member| member.role = "owner" }
    end
end
