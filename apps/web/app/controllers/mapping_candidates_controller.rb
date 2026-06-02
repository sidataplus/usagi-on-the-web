class MappingCandidatesController < ApplicationController
  include ProjectAuthorization
  include MappingCockpit

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

    @mapping = mapping
    respond_to do |format|
      format.turbo_stream do
        advance = Mapping.find_by(id: next_mapping_id(mapping)) if params[:next].present?
        render turbo_stream: cockpit_decision_streams(mapping, advance_to: advance)
      end
      format.html { redirect_to after_candidate_path(mapping), notice: "Candidate applied. Review before approving." }
    end
  end

  private
    def set_candidate
      @candidate = MappingCandidate.find(params[:id])
    end

    def set_mapping
      @mapping = Mapping.find(params[:mapping_id])
    end

    def after_candidate_path(mapping)
      next_id = next_mapping_id(mapping)
      params[:next].present? && next_id.present? ? mapping_path(next_id) : mapping_path(mapping)
    end
end
