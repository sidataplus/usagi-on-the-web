class SourceTerm < ApplicationRecord
  include HasPrefixedId

  prefixed_id "src"

  belongs_to :project
  belongs_to :import_session, optional: true
  has_one :mapping, dependent: :destroy
  has_many :mapping_candidates, dependent: :destroy

  before_validation :normalize_source_name

  validates :source_code, presence: true, uniqueness: { scope: :project_id }
  validates :source_name, presence: true
  validates :source_frequency, numericality: { greater_than_or_equal_to: 0 }

  scope :ordered, -> { order(source_row_number: :asc, id: :asc) }

  def import_batch
    import_session
  end

  def import_batch=(value)
    self.import_session = value
  end

  def row_number
    source_row_number
  end

  def row_number=(value)
    self.source_row_number = value
  end

  def raw_payload
    raw_row
  end

  def raw_payload=(value)
    self.raw_row = value
  end

  def mappings
    Mapping.where(source_term_id: id)
  end

  private
    def normalize_source_name
      self.normalized_source_name = source_name.to_s.strip.downcase.gsub(/\s+/, " ") if source_name.present?
    end
end
