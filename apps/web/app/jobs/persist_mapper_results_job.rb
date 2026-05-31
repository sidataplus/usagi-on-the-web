class PersistMapperResultsJob < ApplicationJob
  queue_as :default

  def perform(engine_job_id)
    engine_job = EngineJob.find(engine_job_id)
    payload = EngineClients::JobsClient.new.results(engine_job.api_job_id)
    engine_job.update!(result: payload)
    Array(payload["items"]).each do |item|
      next unless item["state"].blank? || item["state"] == "succeeded"

      term = SourceTerm.find_by(id: item["source_id"] || item["id"])
      mapping = term&.mapping
      next unless mapping

      candidates = Array(item["candidates"]).map { |candidate| result_from_hash(candidate) }
      Mappings::CandidatePersister.new(mapping: mapping, engine_job: engine_job).persist_results(candidates)
      mapping.update!(candidate_count: mapping.mapping_candidates.count)
    end
  end

  private
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
