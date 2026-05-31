class CreateExportJob < ApplicationJob
  queue_as :default

  def perform(export_id)
    export = Export.find(export_id)
    export.update!(state: "running", started_at: Time.current)
    data = Exports::CsvBuilder.new(export.project, format: export.format).to_csv
    export.artifact.attach(
      io: StringIO.new(data),
      filename: export.file_key.presence || "#{export.project.name.parameterize}-#{export.format}.csv",
      content_type: "text/csv"
    )
    export.update!(state: "succeeded", row_count: export.project.mappings.count, finished_at: Time.current)
  rescue => e
    export&.update!(state: "failed", error: { message: e.message })
    raise
  end
end
