class ImportSession < ApplicationRecord
  include HasPrefixedId

  prefixed_id "imp"

  STATES = %w[pending previewed queued running succeeded succeeded_with_errors failed cancelled].freeze

  belongs_to :project
  belongs_to :created_by, class_name: "User"
  has_one_attached :source_file
  has_many :source_terms, dependent: :destroy

  validates :file_name, presence: true
  validates :state, inclusion: { in: STATES }

  scope :recent_first, -> { order(created_at: :desc, id: :desc) }

  def imported?
    state.in?(%w[succeeded succeeded_with_errors])
  end

  def filename
    file_name
  end

  def filename=(value)
    self.file_name = value
  end

  def status
    state
  end

  def status=(value)
    self.state = value.to_s == "imported" ? "succeeded" : value
  end

  def source_vocabulary
    project&.source_vocabulary
  end

  def source_vocabulary=(_value)
  end

  def failed?
    state == "failed"
  end

  def imported_at
    finished_at
  end

  def imported_at=(value)
    self.finished_at = value
  end

  def row_count
    summary.fetch("rows_seen", source_terms.count).to_i
  end

  def accepted_count
    summary.fetch("rows_imported", source_terms.count).to_i
  end

  def rejected_count
    summary.fetch("rows_failed", 0).to_i
  end

  def accepted_percentage
    total = row_count
    return 0 if total.zero?

    ((accepted_count.to_f / total) * 100).round
  end

  def imported_at
    finished_at
  end

  def imported_at=(value)
    self.finished_at = value
  end
end
