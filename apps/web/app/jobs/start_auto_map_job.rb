require "digest"

class StartAutoMapJob < ApplicationJob
  queue_as :default

  def perform(project_id)
    project = Project.find(project_id)
    items = project.source_terms.ordered.map do |term|
      {
        id: term.id,
        source_code: term.source_code,
        source_name: term.source_name,
        source_frequency: term.source_frequency
      }
    end
    idempotency_key = "mapper-#{project.id}-#{Digest::SHA256.hexdigest(items.to_json)}"

    engine_job = project.engine_jobs.mapper_drugs_batch.to_a.find { |job| job.idempotency_key == idempotency_key }
    if reusable_mirror?(engine_job)
      PollEngineJobJob.perform_later(engine_job.id) if engine_job.api_job_id.present? && engine_job.state.in?(%w[queued starting running])
      return engine_job
    end

    engine_job ||= project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      mode: "thirawat_tachiom",
      candidate_set_id: "candset_#{SecureRandom.hex(8)}",
      input: { item_count: items.size, idempotency_key: idempotency_key }
    )
    engine_job.update!(
      state: "starting",
      error: {},
      result: {},
      api_job_id: nil,
      status_url: nil,
      result_url: nil,
      started_at: Time.current,
      finished_at: nil
    )
    response = EngineClients::MapperClient.new.start_drug_batch_job(project: project, items: items, idempotency_key: idempotency_key)
    engine_job.update!(
      api_job_id: response.fetch("job_id"),
      state: response.fetch("state"),
      status_url: response["status_url"],
      result_url: response["result_url"]
    )
    PollEngineJobJob.perform_later(engine_job.id)
    engine_job
  rescue EngineClients::BaseClient::Error => e
    engine_job&.update!(
      state: "failed",
      error: { code: e.code, message: e.message, details: e.details, request_id: e.request_id }.compact,
      finished_at: Time.current
    )
    raise
  end

  private
    def reusable_mirror?(engine_job)
      return false unless engine_job
      return true if engine_job.state == "succeeded"

      engine_job.api_job_id.present? && engine_job.state.in?(%w[queued starting running])
    end
end
