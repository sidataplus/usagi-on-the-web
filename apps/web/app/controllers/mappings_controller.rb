class MappingsController < ApplicationController
  include ProjectAuthorization
  include MappingCockpit

  before_action :set_project, only: :index
  before_action :set_mapping, except: %i[index bulk_update]

  PER_PAGE = 50
  STATUS_FILTERS = %w[unchecked approved flagged invalid_status invalid].freeze

  # UI sort key -> SQL ordering. source_* columns live on source_terms, so they
  # need the join; match_score/status are on mappings. Keys match the column
  # headers rendered in mappings/_table so the sort indicator lines up.
  SORTS = {
    "source_frequency" => { join: :source_term, column: "source_terms.source_frequency" },
    "source_code"      => { join: :source_term, column: "source_terms.source_code" },
    "source_name"      => { join: :source_term, column: "source_terms.source_name" },
    "match_score"      => { column: "mappings.match_score" },
    "status"           => { column: "mappings.mapping_status" }
  }.freeze

  def index
    authorize_project!(@project, :view)
    @status = params[:status].presence_in(STATUS_FILTERS)
    @query = params[:q].to_s
    @sort, @direction = sort_params
    config = SORTS.fetch(@sort)
    scope = @project.mappings.search(@query).by_status(@status)
    scope = scope.joins(config[:join]) if config[:join]
    scope = scope.order(Arel.sql("#{config[:column]} #{@direction == :asc ? "ASC" : "DESC"}")).order(id: :asc)
    @counts = status_counts(@project)
    @per_page = per_page
    @total = scope.count
    @page = [params[:page].to_i, 1].max
    @total_pages = [(@total.to_f / @per_page).ceil, 1].max
    @mappings = scope.offset((@page - 1) * @per_page).limit(@per_page)
    @latest_engine_job = latest_suggestion_job(@project)
    @next_review_mapping = next_review_mapping(@project)
    @next_review_candidate = @next_review_mapping&.persisted_candidates&.first
    @ready_candidate_count = @project.mappings.unchecked.joins(:mapping_candidates).distinct.count
    @waiting_suggestion_count = @project.mappings.unchecked.left_joins(:mapping_candidates)
                                      .where(mapping_candidates: { id: nil }).count
    @engine_attention_job = engine_attention_job(@project)
    render partial: "mappings/table", locals: table_locals if turbo_frame_request_id == "mappings_table"
  end

  def show
    authorize_project!(@mapping.project, :view)
    @project = @mapping.project
    @cockpit_layout = resolve_cockpit_layout
    @siblings = navigation_for(@mapping)
    @candidates = @mapping.persisted_candidates
    @events = @mapping.project.audit_events.where(subject: @mapping).recent_first
    @comments = @mapping.comments.includes(:user).chronological
  end

  def update
    authorize_project!(@mapping.project, :review)
    if @mapping.update(mapping_params)
      if @mapping.mapping_status_previously_changed?
        @mapping.update!(reviewed_by: current_user, reviewed_at: Time.current)
      end
      audit_mapping("mapping_updated", to: @mapping.mapping_status)
      respond_to do |format|
        format.turbo_stream do
          advance = Mapping.find_by(id: navigation_for(@mapping)[:next_id]) if params[:next].present?
          if advance
            render turbo_stream: cockpit_decision_streams(@mapping, advance_to: advance)
          else
            render :update
          end
        end
        format.html { redirect_to after_mapping_path(@mapping), notice: "Mapping saved." }
      end
    else
      show
      render :show, status: :unprocessable_entity
    end
  end

  def status
    transition_to(params[:status])
    @project = @mapping.project
    @counts = status_counts(@project)
    respond_to do |format|
      format.turbo_stream
      format.html { redirect_to project_mappings_path(@project) }
    end
  end

  def apply_candidate
    candidate = @mapping.mapping_candidates.find(params[:candidate_id])
    authorize_project!(@mapping.project, :review)
    ActiveRecord::Base.transaction do
      @mapping.mapping_candidates.where.not(id: candidate.id).update_all(selected: false)
      candidate.update!(selected: true)
      @mapping.update!(
        target_concept_id: candidate.concept_id,
        target_concept_name: candidate.concept_name,
        target_domain_id: candidate.domain_id,
        target_vocabulary_id: candidate.vocabulary_id,
        target_concept_class_id: candidate.concept_class_id,
        target_standard_concept: candidate.standard_concept,
        target_concept_code: candidate.concept_code,
        match_score: candidate.score,
        search_method: candidate.method
      )
      audit_mapping("candidate_applied", to: candidate.concept_id)
    end
    redirect_to mapping_path(@mapping), notice: "Candidate applied. Review before approving."
  end

  def approve
    transition_to("approved")
    redirect_to mapping_path(@mapping), notice: "Mapping approved."
  end

  def flag
    transition_to("flagged")
    redirect_to mapping_path(@mapping), notice: "Mapping flagged."
  end

  def invalidate
    transition_to("invalid")
    redirect_to mapping_path(@mapping), notice: "Mapping marked invalid."
  end

  def bulk_update
    @mappings = Mapping.where(id: params[:mapping_ids].to_s.split(","))
    @project = @mappings.first&.project
    authorize_project!(@project, :review) if @project
    @mappings.find_each do |mapping|
      from = mapping.mapping_status
      mapping.update!(status: params[:status], reviewed_by: current_user, reviewed_at: Time.current)
      mapping.project.audit_events.create!(
        user: current_user,
        subject: mapping,
        action: "status_changed",
        request_id: Current.request_id,
        metadata: { from: from, to: mapping.mapping_status }
      )
    end
    @counts = status_counts(@project) if @project
    respond_to do |format|
      format.turbo_stream
      format.html { redirect_to(@project ? project_mappings_path(@project) : projects_path) }
    end
  end

  def history
    @events = @mapping.project.audit_events.where(subject: @mapping).recent_first
    render partial: "mappings/history", locals: { events: @events }
  end

  def candidates
    @candidates = @mapping.persisted_candidates
    render partial: "mappings/candidates", locals: { mapping: @mapping, candidates: @candidates }
  end

  def concept_search
    results = EngineClients::SearchClient.new.search_concepts(
      q: params[:q].to_s,
      filters: { domain_id: [@mapping.project.mapping_domain] },
      limit: @mapping.project.settings.fetch("candidate_limit", 20)
    )
    render partial: "mappings/concept_search_results", locals: { mapping: @mapping, results: results, query: params[:q].to_s }
  end

  private
    def set_project
      @project = Project.find(params[:project_id])
    end

    def set_mapping
      @mapping = Mapping.find(params[:id])
    end

    def mapping_params
      params.require(:mapping).permit(
        :status, :equivalence, :target_concept_id, :target_concept_name,
        :target_domain_id, :target_vocabulary_id, :target_concept_class_id,
        :target_standard_concept, :target_concept_code, :concept_id, :concept_name,
        :domain_id, :vocabulary_id, :concept_class_id, :standard_concept, :concept_code
      )
    end

    def transition_to(status)
      authorize_project!(@mapping.project, :review)
      from = @mapping.mapping_status
      @mapping.update!(status: status, reviewed_by: current_user, reviewed_at: Time.current)
      audit_mapping("status_changed", from: from, to: @mapping.mapping_status)
    end

    def audit_mapping(action, from: nil, to: nil)
      @mapping.project.audit_events.create!(
        user: current_user,
        subject: @mapping,
        action: action,
        request_id: Current.request_id,
        metadata: { from: from, to: to }.compact
      )
    end

    def sort_params
      key = params[:sort].presence_in(SORTS.keys) || "source_frequency"
      direction = params[:direction] == "asc" ? :asc : :desc
      [key, direction]
    end

    # Honour the reviewer's saved rows-per-page preference (Settings), default 50.
    def per_page
      value = current_user.prefs["rows_per_page"].to_i
      [ 25, 50, 100, 250 ].include?(value) ? value : PER_PAGE
    end

    def status_counts(project)
      base = project.mappings.group(:mapping_status).count
      {
        "all" => project.mappings.count,
        "unchecked" => base["UNCHECKED"].to_i,
        "approved" => base["APPROVED"].to_i,
        "flagged" => base["FLAGGED"].to_i,
        "invalid_status" => base["INVALID"].to_i
      }
    end

    def latest_suggestion_job(project)
      kinds = project.drug_domain? ? ["mapper_drugs_batch"] : ["hybrid_search_batch"]
      project.engine_jobs.where(kind: kinds).recent_first.first
    end

    def next_review_mapping(project)
      project.mappings.unchecked.joins(:mapping_candidates).ordered.first ||
        project.mappings.unchecked.ordered.first
    end

    def engine_attention_job(project)
      kinds = project.drug_domain? ? ["mapper_drugs_batch"] : ["hybrid_search_batch"]
      project.engine_jobs.where(kind: kinds, state: "failed").recent_first.first ||
        project.engine_jobs.where(kind: kinds, state: "succeeded_with_errors").recent_first.first
    end

    # Persist a toggled cockpit layout so it sticks across mappings/sessions.
    def resolve_cockpit_layout
      requested = params[:layout].presence_in(COCKPIT_LAYOUTS)
      if requested && requested != current_user.prefs["cockpit_layout"]
        current_user.update(preferences: current_user.prefs.merge("cockpit_layout" => requested))
      end
      cockpit_layout
    end

    def after_mapping_path(mapping)
      next_id = navigation_for(mapping)[:next_id]
      params[:next].present? && next_id.present? ? mapping_path(next_id) : mapping_path(mapping)
    end

    def table_locals
      { project: @project, mappings: @mappings, counts: @counts, status: @status, query: @query,
        sort: @sort, direction: @direction, page: @page, total: @total, total_pages: @total_pages,
        per_page: @per_page }
    end
end
