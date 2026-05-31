module EngineClients
  class SearchClient < BaseClient
    class << self
      def http_transport
        HttpTransport.new(base_url: ENV.fetch("SEARCH_API_URL"))
      end
    end

    HYBRID_DEFAULTS = { rrf_k: 60, lexical_top_k: 100, sapbert_top_k: 100 }.freeze

    def status
      return transport.get_json("/search/status") if transport.respond_to?(:get_json)

      { "status" => "ready", "api_version" => "stub", "search" => { "mode" => "stub" } }
    end

    def search_concepts(q:, filters: {}, limit: 20, mode: "hybrid_rrf", hybrid: {})
      if transport.respond_to?(:post_json)
        payload = transport.post_json("/search/concepts", {
          q: q,
          mode: mode,
          limit: limit,
          filters: filters,
          hybrid: HYBRID_DEFAULTS.merge(hybrid)
        })
        provenance = payload["provenance"] || {}
        return payload.fetch("results", []).map { |result| concept_result_from_payload(result, provenance: provenance) }
      end

      transport.search_concepts(q: q, filters: filters, limit: limit, mode: mode, hybrid: hybrid)
    end

    def batch(items:, filters: {}, limit_per_item: 20, mode: "hybrid_rrf", hybrid: {})
      if transport.respond_to?(:post_json)
        return transport.post_json("/search/batch", {
          mode: mode,
          limit_per_item: limit_per_item,
          filters: filters,
          hybrid: HYBRID_DEFAULTS.merge(hybrid),
          items: items
        })
      end

      items.map do |item|
        query = item.fetch(:q) { item.fetch("q") }
        item_filters = filters.merge(item[:filters] || item["filters"] || {})

        {
          source_code: item[:source_code] || item["source_code"],
          q: query,
          results: search_concepts(q: query, filters: item_filters, limit: limit_per_item, mode: mode, hybrid: hybrid)
        }
      end
    end
  end
end
