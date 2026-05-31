module UsagiApi
  # Value object for a concept returned by search / mapper endpoints. Mirrors the
  # Concept search result DTO in docs/api/endpoints.md (§4.2 / §4.3). Ranking
  # fields (score/method/rank) are optional for plain catalog lookups.
  ConceptResult = Data.define(
    :concept_id, :concept_name, :domain_id, :vocabulary_id, :concept_class_id,
    :standard_concept, :concept_code, :score, :method, :rank, :scores, :features,
    :provenance, :warnings
  ) do
    def self.from_concept(concept, score: nil, method: nil, rank: nil)
      new(
        concept_id: concept[:concept_id],
        concept_name: concept[:concept_name],
        domain_id: concept[:domain_id],
        vocabulary_id: concept[:vocabulary_id],
        concept_class_id: concept[:concept_class_id],
        standard_concept: concept[:standard_concept],
        concept_code: concept[:concept_code],
        score: score,
        method: method,
        rank: rank,
        scores: {},
        features: {},
        provenance: {},
        warnings: []
      )
    end

    def score_percent
      score && (score * 100).round
    end
  end
end
