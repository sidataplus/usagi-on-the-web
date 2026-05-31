class MappingCandidatesController < ApplicationController
  include ProjectAuthorization

  before_action :set_candidate, only: :update
  before_action :set_mapping, only: :index

  def index
    authorize_project!(@mapping.project, :view)
    @candidates = @mapping.mapping_candidates.ranked
    render partial: "mappings/candidates", locals: { mapping: @mapping, candidates: @candidates }
  end

  def update
    mapping = @candidate.mapping
    authorize_project!(mapping.project, :review)

    ActiveRecord::Base.transaction do
      mapping.mapping_candidates.where.not(id: @candidate.id).update_all(selected: false)
      @candidate.update!(selected: true)
      mapping.update!(
        target_concept_id: @candidate.concept_id,
        target_concept_name: @candidate.concept_name,
        target_domain_id: @candidate.domain_id,
        target_vocabulary_id: @candidate.vocabulary_id,
        target_concept_class_id: @candidate.concept_class_id,
        target_standard_concept: @candidate.standard_concept,
        target_concept_code: @candidate.concept_code,
        match_score: @candidate.score,
        search_method: @candidate.method,
        candidate_count: mapping.mapping_candidates.count
      )
      mapping.project.audit_events.create!(
        user: current_user,
        subject: mapping,
        action: "candidate_applied",
        request_id: Current.request_id,
        metadata: { candidate_id: @candidate.id, concept_id: @candidate.concept_id, method: @candidate.method }
      )
    end

    redirect_to mapping_path(mapping), notice: "Candidate applied. Review before approving."
  end

  private
    def set_candidate
      @candidate = MappingCandidate.find(params[:id])
    end

    def set_mapping
      @mapping = Mapping.find(params[:mapping_id])
    end
end
