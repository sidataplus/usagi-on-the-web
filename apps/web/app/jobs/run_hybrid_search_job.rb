class RunHybridSearchJob < ApplicationJob
  queue_as :default

  def perform(project_id)
    project = Project.find(project_id)
    mappings = project.mappings.unchecked.includes(:source_term)
    batch_size = ENV.fetch("HYBRID_SEARCH_BATCH_SIZE", 100).to_i.clamp(1, 1_000)
    engine_job = project.engine_jobs.create!(
      kind: "hybrid_search_batch",
      state: "running",
      mode: "hybrid_rrf",
      candidate_set_id: "candset_#{SecureRandom.hex(8)}",
      total: mappings.count
    )

    processed = 0
    failed_items = []
    search_client = EngineClients::SearchClient.new

    Mappings::HybridSearchBatchEnumerator.new(relation: mappings, batch_size: batch_size).each_slice do |mapping_slice|
      response = search_client.batch(
        items: mapping_slice.map { |mapping| batch_item(mapping) },
        filters: { domain_id: [project.mapping_domain] },
        limit_per_item: project.settings.fetch("candidate_limit", 20)
      )

      response_provenance = response.is_a?(Hash) ? (response["provenance"] || response[:provenance] || {}) : {}
      items_by_id = response_items(response).index_by { |item| item.fetch("id").to_s }

      mapping_slice.each do |mapping|
        item = items_by_id[mapping.source_term_id]
        processed += 1
        unless item
          failed_items << {
            "source_code" => mapping.source_code,
            "error" => {
              "code" => "MISSING_BATCH_RESULT",
              "message" => "Search batch response did not include this source term"
            }
          }
          next
        end

        if item["state"] == "failed" || item["error"].present?
          failed_items << {
            "id" => item["id"],
            "source_code" => mapping.source_code,
            "q" => mapping.source_name,
            "error" => item["error"]
          }
          next
        end

        results = Array(item["results"] || item["candidates"]).map { |result| result_from_payload(result, provenance: response_provenance) }
        Mappings::CandidatePersister.new(mapping: mapping, engine_job: engine_job).persist_results(results)
        mapping.update!(candidate_count: mapping.mapping_candidates.count)
      end

      engine_job.update!(processed: processed, failed: failed_items.size)
    end

    engine_job.update!(
      state: failed_items.any? ? "succeeded_with_errors" : "succeeded",
      failed: failed_items.size,
      error: failed_items.any? ? { items: failed_items } : {},
      finished_at: Time.current
    )
  rescue EngineClients::BaseClient::Error => e
    failed_count = engine_job ? [engine_job.total.to_i - processed.to_i, 0].max : 0
    engine_job&.update!(
      state: "failed",
      processed: processed.to_i,
      failed: failed_count,
      error: { code: e.code, message: e.message, request_id: e.request_id },
      finished_at: Time.current
    )
    raise
  end

  private
    def batch_item(mapping)
      {
        id: mapping.source_term_id,
        source_code: mapping.source_code,
        q: mapping.source_name,
        filters: { domain_id: [mapping.domain_id] }
      }
    end

    def response_items(response)
      items = response.is_a?(Hash) ? Array(response["items"] || response[:items]) : Array(response)
      items.map { |item| item.respond_to?(:with_indifferent_access) ? item.with_indifferent_access : item }
    end

    def result_from_payload(result, provenance: {})
      return result if result.is_a?(UsagiApi::ConceptResult)

      concept = result.fetch("concept")
      scores = result["scores"] || {}
      UsagiApi::ConceptResult.from_concept(
        {
          concept_id: concept["concept_id"],
          concept_name: concept["concept_name"],
          domain_id: concept["domain_id"],
          vocabulary_id: concept["vocabulary_id"],
          concept_class_id: concept["concept_class_id"],
          standard_concept: concept["standard_concept"],
          concept_code: concept["concept_code"]
        },
        score: scores["final"] || scores["rrf"] || scores["sapbert"] || scores["tantivy"],
        method: result["method"],
        rank: result["rank"]
      ).with(
        scores: scores,
        features: result["features"] || {},
        provenance: result["provenance"] || provenance,
        warnings: result["warnings"] || []
      )
    end
end
