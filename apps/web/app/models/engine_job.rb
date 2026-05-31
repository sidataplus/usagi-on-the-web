class EngineJob < ApplicationRecord
  include HasPrefixedId

  prefixed_id "ejob"

  KINDS = %w[
    mapper_drugs_batch hybrid_search_batch import_source_file create_export
    catalog_build tantivy_build sapbert_build thirawat_embed_build tachiom_build
  ].freeze
  STATES = %w[queued starting running succeeded succeeded_with_errors failed cancelled].freeze

  belongs_to :project, optional: true
  belongs_to :requested_by, class_name: "User", optional: true
  has_many :mapping_candidates, dependent: :nullify

  validates :kind, inclusion: { in: KINDS }
  validates :state, inclusion: { in: STATES }
  validates :api_job_id, uniqueness: true, allow_nil: true
  validate :idempotency_key_is_unique_within_project_and_kind, if: -> { idempotency_key.present? }

  scope :recent_first, -> { order(created_at: :desc, id: :desc) }
  scope :mapper_drugs_batch, -> { where(kind: "mapper_drugs_batch") }
  scope :hybrid_search_batch, -> { where(kind: "hybrid_search_batch") }

  def status
    state
  end

  def status=(value)
    self.state = value
  end

  def job_type
    kind
  end

  def job_type=(value)
    self.kind = value.to_s == "hybrid_search" ? "hybrid_search_batch" : value
  end

  def engine_job_id
    api_job_id
  end

  def engine_job_id=(value)
    self.api_job_id = value
  end

  def request_payload
    input
  end

  def request_payload=(value)
    self.input = value
  end

  def result_payload
    result
  end

  def result_payload=(value)
    self.result = value
  end

  def idempotency_key
    input["idempotency_key"]
  end

  def idempotency_key=(value)
    self.input = input.merge("idempotency_key" => value)
  end

  private
    def idempotency_key_is_unique_within_project_and_kind
      duplicate = self.class.where(project_id: project_id, kind: kind)
                            .where.not(id: id)
                            .to_a
                            .any? { |job| job.idempotency_key == idempotency_key }
      errors.add(:idempotency_key, "has already been taken") if duplicate
    end
end
