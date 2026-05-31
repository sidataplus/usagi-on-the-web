module EngineClients
  # Deterministic local transport that mirrors the shape of the Rust API while
  # endpoints are still under active development in usagi-on-the-web.
  class StubTransport
    def search_concepts(q:, filters: {}, limit: 20, mode: "hybrid_rrf", hybrid: {})
      concepts = UsagiApi::SampleConcepts.search(q, limit: limit * 3)
      concepts = fallback_concepts(q, limit: limit * 3) if concepts.empty?
      concepts = filter_concepts(concepts, filters)

      concepts.first(limit).each_with_index.map do |concept, index|
        UsagiApi::ConceptResult.from_concept(
          concept,
          score: deterministic_score(q, concept, index),
          method: mode,
          rank: index + 1
        )
      end
    end

    def drug_candidates(source_name:, source_code: nil, limit: 10, candidate_top_k: 200, rerank_top_n: 100, post_rank: {})
      search_concepts(
        q: source_name.to_s,
        filters: { domain_id: ["Drug"] },
        limit: limit,
        mode: "thirawat_tachiom_bimaxsim_tiebreak"
      )
    end

    def concept(concept_id)
      found = UsagiApi::SampleConcepts.find(concept_id)
      return UsagiApi::ConceptResult.from_concept(found) if found

      raise BaseClient::Error.new(
        code: "NOT_FOUND",
        message: "Concept #{concept_id} was not found",
        details: { "concept_id" => concept_id.to_i },
        status: 404
      )
    end

    private
      def filter_concepts(concepts, filters)
        domain_ids = values_for(filters, :domain_id)
        vocabulary_ids = values_for(filters, :vocabulary_id)
        concept_class_ids = values_for(filters, :concept_class_id)

        concepts = concepts.select { |concept| domain_ids.include?(concept[:domain_id]) } if domain_ids.any?
        concepts = concepts.select { |concept| vocabulary_ids.include?(concept[:vocabulary_id]) } if vocabulary_ids.any?
        concepts = concepts.select { |concept| concept_class_ids.include?(concept[:concept_class_id]) } if concept_class_ids.any?
        concepts
      end

      def values_for(filters, key)
        Array(filters[key] || filters[key.to_s]).compact
      end

      def fallback_concepts(query, limit:)
        query_terms = normalized_terms(query)
        return UsagiApi::SampleConcepts.all.first(limit) if query_terms.empty?

        UsagiApi::SampleConcepts.all
          .map { |concept| [concept, (query_terms & normalized_terms(concept[:concept_name])).length] }
          .select { |_concept, overlap| overlap.positive? }
          .sort_by { |concept, overlap| [-overlap, concept[:concept_name].length, concept[:concept_id]] }
          .first(limit)
          .map(&:first)
      end

      def normalized_terms(value)
        value.to_s
          .downcase
          .gsub(/\bhcl\b/, "hydrochloride")
          .gsub(/\bcap\b/, "capsule")
          .gsub(/\btab\b/, "tablet")
          .gsub(/(\d+)(mg|mcg|g|ml)\b/, "\\1 \\2")
          .scan(/[a-z0-9]+/)
      end

      # Stable pseudo-score in [0.45, 0.98] from the query/concept pair so the
      # same search always ranks the same way.
      def deterministic_score(query, concept, index)
        seed = "#{query}|#{concept[:concept_id]}".each_char.sum(&:ord)
        base = 0.98 - (index * 0.04) - ((seed % 13) * 0.01)
        base.clamp(0.45, 0.98).round(3)
      end
  end
end
