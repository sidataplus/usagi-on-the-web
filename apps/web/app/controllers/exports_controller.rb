class ExportsController < ApplicationController
  include ProjectAuthorization

  before_action :set_project
  before_action :set_export, only: :show

  def index
    authorize_project!(@project, :view)
    @exports = @project.exports.recent_first
  end

  def create
    authorize_project!(@project, :export)
    export = @project.exports.create!(
      requested_by: current_user,
      format: params[:format_option].presence_in(Export::FORMATS) || "review_csv",
      state: "queued",
      filters: export_filters,
      file_key: "#{@project.name.parameterize}-#{Time.current.to_i}.#{file_extension(params[:format_option])}"
    )
    @project.audit_events.create!(
      user: current_user,
      subject: export,
      action: "export_requested",
      request_id: Current.request_id,
      metadata: { format: export.format }
    )
    CreateExportJob.perform_later(export.id)
    redirect_to project_export_path(@project, export), notice: "Export queued."
  end

  def show
    authorize_project!(@project, :export)
    respond_to do |format|
      format.html
      format.csv do
        export = @export || create_inline_export
        data = Exports::CsvBuilder.new(@project, format: export.format).to_csv
        send_data data, filename: export.file_key, type: "text/csv"
      end
      format.jsonl do
        export = @export || create_inline_export(format: "candidate_jsonl", extension: "jsonl")
        data = Exports::CsvBuilder.new(@project, format: export.format).to_jsonl
        send_data data, filename: export.file_key, type: "application/x-ndjson"
      end
    end
  end

  private
    def set_project
      @project = Project.find(params[:project_id])
    end

    def set_export
      @export = @project.exports.find_by(id: params[:id])
    end

    def create_inline_export(format: "review_csv", extension: "csv")
      builder = Exports::CsvBuilder.new(@project, format: format)
      @project.exports.create!(
        requested_by: current_user,
        state: "succeeded",
        format: format,
        row_count: builder.row_count,
        file_key: "#{@project.name.parameterize}-mappings.#{extension}",
        finished_at: Time.current
      ).tap do |export|
        @project.audit_events.create!(
          user: current_user,
          subject: export,
          action: "export_generated",
          request_id: Current.request_id,
          metadata: { format: export.format, row_count: export.row_count }
        )
      end
    end

    def export_filters
      params.fetch(:filters, {}).permit(:status, :domain_id, :vocabulary_id).to_h
    end

    def file_extension(format)
      format.to_s.end_with?("_jsonl") ? "jsonl" : "csv"
    end
end
