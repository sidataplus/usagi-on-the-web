module EngineClients
  class JobsClient < BaseClient
    class << self
      def http_transport
        HttpTransport.new(base_url: ENV.fetch("JOBS_API_URL", ENV.fetch("MAPPER_API_URL", "http://localhost:8790")))
      end
    end

    def status(engine_job_or_id)
      if engine_job_or_id.respond_to?(:api_job_id)
        api_job_id = engine_job_or_id.api_job_id
        return local_status(engine_job_or_id) if api_job_id.blank?
      else
        api_job_id = engine_job_or_id
      end

      return transport.get_json("/jobs/#{api_job_id}") if transport.respond_to?(:get_json)

      local_status(engine_job_or_id)
    end

    def events(api_job_id)
      return transport.get_json("/jobs/#{api_job_id}/events") if transport.respond_to?(:get_json)

      { "events" => [] }
    end

    def results(api_job_id)
      return { "job_id" => api_job_id, "state" => "succeeded", "items" => [] } unless transport.respond_to?(:get_json)

      payload = transport.get_json("/jobs/#{api_job_id}/results")
      # The live engine returns an artifact envelope and serves the per-term
      # results as JSONL; the stub/contract path returns inline items already.
      return payload if Array(payload["items"]).any?
      return payload unless payload["artifact"] && transport.respond_to?(:get_jsonl)

      payload.merge("items" => result_items(api_job_id))
    end

    private
      def local_status(engine_job)
        return { "id" => nil, "state" => nil } unless engine_job.respond_to?(:id)

      {
        "id" => engine_job.api_job_id || engine_job.id,
        "state" => engine_job.state,
        "kind" => engine_job.kind,
        "processed" => engine_job.processed,
        "total" => engine_job.total,
        "failed" => engine_job.failed,
        "result" => engine_job.result,
        "error" => engine_job.error,
        "stage" => engine_job.stage,
        "status_url" => engine_job.status_url,
        "result_url" => engine_job.result_url
      }
      end

      def result_items(api_job_id)
        transport.get_jsonl("/jobs/#{api_job_id}/results")
      end
  end
end
