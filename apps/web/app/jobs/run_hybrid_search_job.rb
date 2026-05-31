class RunHybridSearchJob < ApplicationJob
  queue_as :default

  def perform(project_id)
    project = Project.find(project_id)
    engine_job = project.engine_jobs.create!(
      kind: "hybrid_search_batch",
      state: "running",
      mode: "hybrid_rrf",
      candidate_set_id: "candset_#{SecureRandom.hex(8)}",
      total: project.source_terms.count
    )

    project.mappings.includes(:source_term).find_each.with_index(1) do |mapping, index|
      next unless mapping.unchecked?

      results = EngineClients::SearchClient.new.search_concepts(
        q: mapping.source_name,
        filters: { domain_id: [project.mapping_domain] },
        limit: project.settings.fetch("candidate_limit", 20)
      )
      Mappings::CandidatePersister.new(mapping: mapping, engine_job: engine_job).persist_results(results)
      mapping.update!(candidate_count: mapping.mapping_candidates.count)
      engine_job.update!(processed: index)
    end

    engine_job.update!(state: "succeeded", finished_at: Time.current)
  rescue EngineClients::BaseClient::Error => e
    engine_job&.update!(state: "failed", error: { code: e.code, message: e.message, request_id: e.request_id })
    raise
  end
end
