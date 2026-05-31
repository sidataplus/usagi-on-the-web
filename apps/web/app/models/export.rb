class Export < ApplicationRecord
  include HasPrefixedId

  prefixed_id "exp"

  FORMATS = %w[review_csv source_to_concept_map_csv candidate_csv candidate_jsonl audit_csv csv].freeze
  STATES = %w[queued running succeeded failed cancelled].freeze

  belongs_to :project
  belongs_to :requested_by, class_name: "User", optional: true
  has_one_attached :artifact

  validates :format, inclusion: { in: FORMATS }
  validates :state, inclusion: { in: STATES }

  scope :recent_first, -> { order(created_at: :desc, id: :desc) }
end
