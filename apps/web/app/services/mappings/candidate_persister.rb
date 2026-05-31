module Mappings
  class CandidatePersister
    def initialize(mapping:, engine_job: nil, candidate_set_id: nil)
      @mapping = mapping
      @source_term = mapping.source_term
      @project = mapping.project
      @engine_job = engine_job
      @candidate_set_id = candidate_set_id || engine_job&.candidate_set_id || "candset_#{SecureRandom.hex(8)}"
    end

    def persist_results(results)
      Array(results).map.with_index(1) do |result, index|
        persist_result(result, index)
      end
    end

    private
      attr_reader :mapping, :source_term, :project, :engine_job, :candidate_set_id

      def persist_result(result, index)
        attributes = attributes_for(result, index)
        candidate = MappingCandidate.find_or_initialize_by(
          mapping: mapping,
          concept_id: attributes.fetch(:concept_id),
          method: attributes.fetch(:method)
        )
        candidate.assign_attributes(attributes)
        candidate.save!
        candidate
      end

      def attributes_for(result, index)
        scores = result.respond_to?(:scores) ? result.scores : {}
        {
          project: project,
          source_term: source_term,
          engine_job: engine_job,
          rank: result.rank || index,
          concept_id: result.concept_id,
          concept_name: result.concept_name,
          domain_id: result.domain_id,
          vocabulary_id: result.vocabulary_id,
          concept_class_id: result.concept_class_id,
          standard_concept: result.standard_concept,
          concept_code: result.concept_code,
          tantivy_score: scores["tantivy"],
          sapbert_score: scores["sapbert"],
          rrf_score: scores["rrf"],
          tachiom_maxsim_score: scores["tachiom_maxsim"],
          bimaxsim_score: scores["bimaxsim"],
          tie_breaker_score: scores["tie_breaker"],
          final_score: result.score || scores["final"],
          method: result.method || "manual",
          candidate_set_id: candidate_set_id,
          features: result.respond_to?(:features) ? result.features : {},
          provenance: result.respond_to?(:provenance) ? result.provenance : {},
          warnings: result.respond_to?(:warnings) ? result.warnings : []
        }
      end
  end
end
