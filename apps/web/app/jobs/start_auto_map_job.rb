class StartAutoMapJob < ApplicationJob
  queue_as :default

  def perform(project_id)
    project = Project.find(project_id)
    engine_job = project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "starting",
      mode: "thirawat_tachiom",
      candidate_set_id: "candset_#{SecureRandom.hex(8)}",
      input: { item_count: project.source_terms.count }
    )
    items = project.source_terms.ordered.map do |term|
      {
        id: term.id,
        source_code: term.source_code,
        source_name: term.source_name,
        source_frequency: term.source_frequency
      }
    end
    response = EngineClients::MapperClient.new.start_drug_batch_job(project: project, items: items)
    engine_job.update!(
      api_job_id: response.fetch("job_id"),
      state: response.fetch("state"),
      status_url: response["status_url"],
      result_url: response["result_url"]
    )
    PollEngineJobJob.perform_later(engine_job.id)
  end
end
