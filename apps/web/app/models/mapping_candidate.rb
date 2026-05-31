class MappingCandidate < ApplicationRecord
  include HasPrefixedId

  prefixed_id "cand"

  belongs_to :project
  belongs_to :source_term
  belongs_to :mapping
  belongs_to :engine_job, optional: true
  belongs_to :engine_job, optional: true

  validates :concept_id, :concept_name, :rank, :method, presence: true

  scope :ranked, -> { order(rank: :asc, final_score: :desc, id: :asc) }
  scope :selected, -> { where(selected: true) }

  def score
    final_score || rrf_score || bimaxsim_score || tachiom_maxsim_score || sapbert_score || tantivy_score
  end

  def score=(value)
    self.final_score = value
  end
end
