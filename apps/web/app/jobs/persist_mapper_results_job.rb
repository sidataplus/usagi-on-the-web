class PersistMapperResultsJob < ApplicationJob
  queue_as :default

  def perform(engine_job_id)
    engine_job = EngineJob.find(engine_job_id)
    payload = EngineClients::JobsClient.new.results(engine_job.api_job_id)
    engine_job.update!(result: payload)
    failed_items = []

    Array(payload["items"]).each do |raw_item|
      item = raw_item.respond_to?(:with_indifferent_access) ? raw_item.with_indifferent_access : raw_item
      term = SourceTerm.find_by(id: item["source_id"] || item["id"])
      if item["error"].present? || item["state"].present? && item["state"] != "succeeded"
        failed_items << failed_item_payload(item, term: term)
        next
      end

      mapping = term&.mapping
      next unless mapping

      candidates = Array(item["candidates"]).map { |candidate| result_from_hash(candidate) }
      Mappings::CandidatePersister.new(mapping: mapping, engine_job: engine_job).persist_results(candidates)
      mapping.update!(candidate_count: mapping.mapping_candidates.count)
    end

    record_failed_items!(engine_job, failed_items) if failed_items.any?
  rescue EngineClients::BaseClient::Error => e
    engine_job&.update!(
      state: "failed",
      error: { code: e.code, message: e.message, details: e.details, request_id: e.request_id }.compact,
      finished_at: Time.current
    )
    raise
  end

  private
    def failed_item_payload(item, term:)
      {
        "id" => item["id"],
        "source_code" => item["source_code"] || term&.source_code,
        "q" => item["q"] || item["source_name"] || term&.source_name,
        "error" => item["error"] || { "code" => "ITEM_FAILED", "message" => "Mapper item failed" }
      }.compact
    end

    def record_failed_items!(engine_job, failed_items)
      engine_job.update!(
        state: engine_job.state == "succeeded" ? "succeeded_with_errors" : engine_job.state,
        failed: [engine_job.failed.to_i, failed_items.size].max,
        error: engine_job.error.merge("items" => failed_items)
      )
    end

    def result_from_hash(candidate)
      concept = candidate.fetch("concept")
      scores = candidate["scores"] || {}
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
        score: scores["final"],
        method: candidate["method"],
        rank: candidate["rank"]
      ).with(scores: scores, features: candidate["features"] || {}, provenance: candidate["provenance"] || {}, warnings: candidate["warnings"] || [])
    end
end
