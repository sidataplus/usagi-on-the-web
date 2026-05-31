class CreateExportJob < ApplicationJob
  queue_as :default

  def perform(export_id)
    export = Export.find(export_id)
    export.update!(state: "running", started_at: Time.current)
    builder = Exports::CsvBuilder.new(export.project, format: export.format)
    jsonl = export.format.to_s.end_with?("_jsonl")
    data = jsonl ? builder.to_jsonl : builder.to_csv
    export.artifact.attach(
      io: StringIO.new(data),
      filename: export.file_key.presence || "#{export.project.name.parameterize}-#{export.format}.#{jsonl ? "jsonl" : "csv"}",
      content_type: jsonl ? "application/x-ndjson" : "text/csv"
    )
    export.update!(state: "succeeded", row_count: builder.row_count, finished_at: Time.current)
  rescue => e
    export&.update!(state: "failed", error: { message: e.message })
    raise
  end
end
