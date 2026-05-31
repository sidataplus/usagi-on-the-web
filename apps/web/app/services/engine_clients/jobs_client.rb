module EngineClients
  class JobsClient < BaseClient
    class << self
      def http_transport
        HttpTransport.new(base_url: ENV.fetch("JOBS_API_URL", ENV.fetch("MAPPER_API_URL", "http://localhost:8790")))
      end
    end

    def status(engine_job_or_id)
      api_job_id = engine_job_or_id.respond_to?(:api_job_id) ? engine_job_or_id.api_job_id : engine_job_or_id
      return transport.get_json("/jobs/#{api_job_id}") if transport.respond_to?(:get_json)

      engine_job = engine_job_or_id
      {
        "id" => engine_job.api_job_id || engine_job.id,
        "state" => engine_job.state,
        "kind" => engine_job.kind,
        "processed" => engine_job.processed,
        "total" => engine_job.total,
        "failed" => engine_job.failed,
        "result" => engine_job.result,
        "error" => engine_job.error
      }
    end

    def events(api_job_id)
      return transport.get_json("/jobs/#{api_job_id}/events") if transport.respond_to?(:get_json)

      { "events" => [] }
    end

    def results(api_job_id)
      return transport.get_json("/jobs/#{api_job_id}/results") if transport.respond_to?(:get_json)

      { "job_id" => api_job_id, "state" => "succeeded", "items" => [] }
    end
  end
end
