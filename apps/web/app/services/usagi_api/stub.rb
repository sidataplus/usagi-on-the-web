module UsagiApi
  # In-process stand-in for the Rust usagi-api while it is not yet wired up.
  # Returns deterministic, seeded-looking data so the UI renders. Scores are
  # derived from the query so they are stable across requests (no randomness).
  module Stub
    module_function

    # Mirrors POST /search/concepts (hybrid_rrf). Returns ranked ConceptResults.
    def search_concepts(query:, domain: nil, vocabulary: nil, limit: 20)
      concepts = SampleConcepts.search(query, limit: limit * 2)
      concepts = concepts.select { |c| c[:domain_id] == domain } if domain.present?
      concepts = concepts.select { |c| Array(vocabulary).include?(c[:vocabulary_id]) } if vocabulary.present?

      concepts.first(limit).each_with_index.map do |concept, index|
        ConceptResult.from_concept(
          concept,
          score: deterministic_score(query, concept, index),
          method: "hybrid_rrf",
          rank: index + 1
        )
      end
    end

    # Mirrors POST /mapper/drugs/query — top-N candidates for a source string.
    def candidates(source_name:, domain: nil, limit: 10)
      search_concepts(query: source_name.to_s, domain: domain, limit: limit).map do |result|
        result.with(method: "thirawat_tachiom_bimaxsim_tiebreak")
      end
    end

    # Mirrors GET /catalog/concepts/:concept_id.
    def concept(concept_id)
      found = SampleConcepts.find(concept_id)
      found && ConceptResult.from_concept(found)
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
