class Mapping < ApplicationRecord
  include HasPrefixedId

  prefixed_id "map"

  STATUSES = {
    "unchecked" => "UNCHECKED",
    "approved" => "APPROVED",
    "flagged" => "FLAGGED",
    "invalid" => "INVALID",
    "invalid_status" => "INVALID"
  }.freeze
  EQUIVALENCES = ["Equivalent", "Narrower", "Broader", "Related", "No match", "Unclear"].freeze
  EQUIVALENCE_LABELS = EQUIVALENCES.index_with(&:itself).freeze

  belongs_to :project
  belongs_to :source_term
  belongs_to :reviewed_by, class_name: "User", optional: true
  has_many :mapping_candidates, dependent: :destroy
  has_many :comments, dependent: :destroy
  has_many :mapping_comments, class_name: "Comment", dependent: :destroy
  has_many :audit_events, as: :subject, dependent: :nullify
  has_many :mapping_events, -> { where(subject_type: "Mapping") }, class_name: "AuditEvent",
           foreign_key: :subject_id, primary_key: :id

  validates :mapping_status, inclusion: { in: STATUSES.values.uniq }
  validates :source_term_id, uniqueness: true

  scope :unchecked, -> { where(mapping_status: "UNCHECKED") }
  scope :approved, -> { where(mapping_status: "APPROVED") }
  scope :flagged, -> { where(mapping_status: "FLAGGED") }
  scope :invalid_status, -> { where(mapping_status: "INVALID") }
  scope :by_status, ->(value) { value.present? ? where(mapping_status: normalize_status(value)) : all }
  scope :search, ->(query) {
    if query.present?
      joins(:source_term).where(
        "source_terms.source_code LIKE :term OR source_terms.source_name LIKE :term OR mappings.target_concept_name LIKE :term",
        term: "%#{sanitize_sql_like(query.strip)}%"
      )
    else
      all
    end
  }
  scope :ordered, -> { joins(:source_term).order("source_terms.source_frequency DESC", "source_terms.source_code ASC") }

  def self.statuses
    STATUSES
  end

  def self.normalize_status(value)
    STATUSES.fetch(value.to_s, value.to_s.upcase)
  end

  def status
    case mapping_status
    when "APPROVED" then "approved"
    when "FLAGGED" then "flagged"
    when "INVALID" then "invalid_status"
    else "unchecked"
    end
  end

  def status=(value)
    self.mapping_status = self.class.normalize_status(value)
  end

  def status_label
    mapping_status.to_s.humanize
  end

  def approved?
    mapping_status == "APPROVED"
  end

  def flagged?
    mapping_status == "FLAGGED"
  end

  def unchecked?
    mapping_status == "UNCHECKED"
  end

  def invalid_status?
    mapping_status == "INVALID"
  end

  def source_code
    source_term&.source_code
  end

  def source_name
    source_term&.source_name
  end

  def source_frequency
    source_term&.source_frequency.to_i
  end

  def domain_id
    target_domain_id || source_term&.source_domain_hint || project.mapping_domain
  end

  def domain_id=(value)
    self.target_domain_id = value
  end

  def concept_id
    target_concept_id
  end

  def concept_id=(value)
    self.target_concept_id = value
  end

  def concept_name
    target_concept_name
  end

  def concept_name=(value)
    self.target_concept_name = value
  end

  def vocabulary_id
    target_vocabulary_id
  end

  def vocabulary_id=(value)
    self.target_vocabulary_id = value
  end

  def concept_class_id
    target_concept_class_id
  end

  def concept_class_id=(value)
    self.target_concept_class_id = value
  end

  def standard_concept
    target_standard_concept
  end

  def standard_concept=(value)
    self.target_standard_concept = value
  end

  def concept_code
    target_concept_code
  end

  def concept_code=(value)
    self.target_concept_code = value
  end

  def mapped?
    target_concept_id.present?
  end

  def persisted_candidates
    mapping_candidates.order(rank: :asc, final_score: :desc, id: :asc)
  end

  def equivalence_label
    equivalence
  end

  def score_band
    return nil if match_score.nil?

    return :high if match_score >= 0.8
    return :medium if match_score >= 0.5

    :low
  end
end
