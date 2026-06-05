class Export < ApplicationRecord
  include HasPrefixedId

  prefixed_id "exp"

  FORMATS = %w[review_csv usagi_csv source_to_concept_map_csv candidate_csv candidate_jsonl audit_csv csv].freeze
  STATES = %w[queued running succeeded failed cancelled].freeze

  # Human labels for the formats a reviewer can pick. Excludes the internal bare
  # "csv" alias used only by inline downloads.
  FORMAT_LABELS = {
    "review_csv" => "Review CSV",
    "usagi_csv" => "USAGI CSV",
    "source_to_concept_map_csv" => "SOURCE_TO_CONCEPT_MAP CSV",
    "candidate_csv" => "Candidate provenance (CSV)",
    "candidate_jsonl" => "Candidate provenance (JSONL)",
    "audit_csv" => "Audit trail CSV"
  }.freeze

  # [label, value] pairs for the export format dropdown.
  def self.selectable_formats
    FORMAT_LABELS.map { |value, label| [ label, value ] }
  end

  def format_label
    FORMAT_LABELS.fetch(format, format.to_s.humanize)
  end

  belongs_to :project
  belongs_to :requested_by, class_name: "User", optional: true
  has_one_attached :artifact

  validates :format, inclusion: { in: FORMATS }
  validates :state, inclusion: { in: STATES }

  scope :recent_first, -> { order(created_at: :desc, id: :desc) }
end
