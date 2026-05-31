module EngineClients
  class CatalogClient < BaseClient
    class << self
      def http_transport
        HttpTransport.new(base_url: ENV.fetch("CATALOG_API_URL"))
      end
    end

    def status
      return transport.get_json("/catalog/status") if transport.respond_to?(:get_json)

      {
        "status" => "ready",
        "api_version" => "stub",
        "catalog" => {
          "artifact_id" => "stub-catalog",
          "vocabulary_version" => "stub",
          "concept_count" => UsagiApi::SampleConcepts.all.size
        }
      }
    end

    def concept(concept_id)
      if transport.respond_to?(:get_json)
        payload = transport.get_json("/catalog/concepts/#{concept_id}")
        concept = payload.fetch("concept")
        return UsagiApi::ConceptResult.from_concept(
          {
            concept_id: concept["concept_id"],
            concept_name: concept["concept_name"],
            domain_id: concept["domain_id"],
            vocabulary_id: concept["vocabulary_id"],
            concept_class_id: concept["concept_class_id"],
            standard_concept: concept["standard_concept"],
            concept_code: concept["concept_code"]
          }
        ).with(provenance: payload["provenance"] || {})
      end

      transport.concept(concept_id)
    end
  end
end
