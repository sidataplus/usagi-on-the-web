module UsagiApi
  # Compatibility facade used by the scaffold UI. Internally this now delegates
  # through EngineClients, which mirrors the API-first Rust service boundaries.
  #
  # Endpoint map (future):
  #   search_concepts -> POST /search/concepts
  #   candidates      -> POST /mapper/drugs/query for Drug, /search/concepts otherwise
  #   concept         -> GET  /catalog/concepts/:concept_id
  class Client
    class << self
      def search_concepts(query:, domain: nil, vocabulary: nil, limit: 20)
        EngineClients::SearchClient.new.search_concepts(
          q: query,
          filters: concept_filters(domain: domain, vocabulary: vocabulary),
          limit: limit
        )
      end

      def candidates(source_name:, domain: nil, limit: 10)
        if domain.blank? || domain == "Drug"
          EngineClients::MapperClient.new.drug_candidates(source_name: source_name, limit: limit)
        else
          search_concepts(query: source_name, domain: domain, limit: limit)
        end
      end

      def concept(concept_id)
        EngineClients::CatalogClient.new.concept(concept_id)
      end

      private
        def concept_filters(domain:, vocabulary:)
          filters = {}
          filters[:domain_id] = Array(domain) if domain.present?
          filters[:vocabulary_id] = Array(vocabulary) if vocabulary.present?
          filters
        end
    end
  end
end
