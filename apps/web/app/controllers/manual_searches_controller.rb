class ManualSearchesController < ApplicationController
  include ProjectAuthorization

  before_action :set_mapping

  def create
    authorize_project!(@mapping.project, :review)
    query = params[:q].presence || @mapping.source_name
    @engine_job = @mapping.project.engine_jobs.create!(
      kind: @mapping.project.drug_domain? ? "mapper_drugs_batch" : "hybrid_search_batch",
      state: "succeeded",
      mode: @mapping.project.drug_domain? ? "thirawat_tachiom" : "hybrid_rrf",
      input: { q: query, source_term_id: @mapping.source_term_id }
    )
    results = engine_results(query)
    @candidates = Mappings::CandidatePersister.new(mapping: @mapping, engine_job: @engine_job).persist_results(results)
    @mapping.update!(candidate_count: @mapping.mapping_candidates.count)
    @mapping.project.audit_events.create!(
      user: current_user,
      subject: @mapping,
      action: "manual_search",
      request_id: Current.request_id,
      metadata: { q: query, candidate_count: @candidates.size, engine_job_id: @engine_job.id }
    )

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: turbo_stream.replace(
          helpers.dom_id(@mapping, :candidate_panel),
          partial: "mappings/candidate_panel",
          locals: { mapping: @mapping, candidates: @candidates }
        )
      end
      format.html { redirect_to mapping_path(@mapping), notice: "Search finished." }
    end
  end

  private
    def set_mapping
      @mapping = Mapping.find(params[:mapping_id])
    end

    def engine_results(query)
      if @mapping.project.drug_domain?
        EngineClients::MapperClient.new.drug_candidates(source_name: query, source_code: @mapping.source_code, limit: candidate_limit)
      else
        EngineClients::SearchClient.new.search_concepts(
          q: query,
          filters: { domain_id: [@mapping.project.mapping_domain] },
          limit: candidate_limit
        )
      end
    end

    def candidate_limit
      @mapping.project.settings.fetch("candidate_limit", 20)
    end
end
