require "digest"
require "securerandom"

module EngineClients
  class MapperClient < BaseClient
    class << self
      def http_transport
        HttpTransport.new(base_url: ENV.fetch("MAPPER_API_URL"))
      end
    end

    THIRAWAT_DEFAULTS = {
      mode: "thirawat_tachiom",
      candidate_top_k: 200,
      rerank_top_n: 100,
      post_rank: { mode: "tiebreak", epsilon: 0.01, top_n: 100 }
    }.freeze

    def status
      return transport.get_json("/mapper/status") if transport.respond_to?(:get_json)

      { "status" => "ready", "api_version" => "stub", "mapper" => { "mode" => "stub" } }
    end

    def drug_candidates(source_name:, source_code: nil, limit: 10, candidate_top_k: 200, rerank_top_n: 100, post_rank: {})
      if transport.respond_to?(:post_json)
        payload = transport.post_json("/mapper/drugs/query", THIRAWAT_DEFAULTS.merge(
          source_name: source_name,
          source_code: source_code,
          limit: limit,
          candidate_top_k: candidate_top_k,
          rerank_top_n: rerank_top_n,
          post_rank: THIRAWAT_DEFAULTS[:post_rank].merge(post_rank)
        ))
        provenance = payload["provenance"] || {}
        return payload.fetch("candidates", []).map { |result| concept_result_from_payload(result, provenance: provenance) }
      end

      transport.drug_candidates(
        source_name: source_name,
        source_code: source_code,
        limit: limit,
        candidate_top_k: candidate_top_k,
        rerank_top_n: rerank_top_n,
        post_rank: post_rank
      )
    end

    alias query_drug drug_candidates

    def start_drug_batch_job(project:, items:, mode: "thirawat_tachiom", idempotency_key: nil)
      payload = {
        idempotency_key: idempotency_key || "mapper-#{project.id}-#{Digest::SHA256.hexdigest(items.to_json)}",
        mode: mode,
        candidate_top_k: 200,
        rerank_top_n: 100,
        limit: project.settings.fetch("candidate_limit", 20),
        post_rank: THIRAWAT_DEFAULTS[:post_rank],
        items: items
      }
      return transport.post_json("/mapper/drugs/batch-job", payload) if transport.respond_to?(:post_json)

      {
        "job_id" => "job_stub_#{SecureRandom.hex(8)}",
        "state" => "queued",
        "status_url" => "/jobs/job_stub",
        "result_url" => "/jobs/job_stub/results"
      }
    end
  end
end
