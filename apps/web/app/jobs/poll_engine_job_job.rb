class PollEngineJobJob < ApplicationJob
  queue_as :default

  def perform(engine_job_id)
    engine_job = EngineJob.find(engine_job_id)
    status = EngineClients::JobsClient.new.status(engine_job)
    engine_job.update!(
      state: status.fetch("state", engine_job.state),
      stage: status["stage"],
      processed: status.fetch("processed", 0),
      total: status.fetch("total", 0),
      failed: status.fetch("failed", 0),
      started_at: parse_time(status["started_at"]),
      finished_at: parse_time(status["finished_at"])
    )

    case engine_job.state
    when "queued", "starting", "running"
      self.class.set(wait: ENV.fetch("ENGINE_API_JOB_POLL_INTERVAL_SECONDS", 2).to_i.seconds).perform_later(engine_job.id)
    when "succeeded", "succeeded_with_errors"
      PersistMapperResultsJob.perform_later(engine_job.id)
    end
  end

  private
    def parse_time(value)
      value.present? ? Time.zone.parse(value.to_s) : nil
    end
end
