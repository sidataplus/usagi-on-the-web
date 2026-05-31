require "csv"
require "roo"
require "tempfile"

class ImportSessionsController < ApplicationController
  include ProjectAuthorization

  before_action :set_project
  before_action :set_import_session, only: %i[show confirm]

  def index
    authorize_project!(@project, :view)
    @imports = @project.import_sessions.recent_first
    render "imports/index"
  end

  def new
    authorize_project!(@project, :import)
    @import_session = @project.import_sessions.new(file_name: "source_terms.csv")
    @import_batch = @import_session
    render "imports/new"
  end

  def create
    authorize_project!(@project, :import)
    @import_session = @project.import_sessions.new(import_params.except(:source_file))
    @import_batch = @import_session
    @import_session.created_by = current_user
    attach_source_file
    rows = parsed_rows(@import_session)

    ActiveRecord::Base.transaction do
      @import_session.state = "succeeded"
      @import_session.summary = { rows_seen: rows.size, rows_imported: rows.size, rows_failed: 0 }
      @import_session.finished_at = Time.current
      @import_session.save!

      rows.each do |row|
        source_term = @import_session.source_terms.create!(
          project: @project,
          source_code: row.fetch(:source_code),
          source_name: row.fetch(:source_name),
          source_frequency: row.fetch(:source_frequency),
          source_domain_hint: row[:source_domain_hint],
          source_vocabulary: @project.source_vocabulary,
          source_row_number: row.fetch(:source_row_number),
          raw_row: row.fetch(:raw_row)
        )
        source_term.create_mapping!(project: @project, mapping_status: "UNCHECKED")
      end

      @project.audit_events.create!(
        user: current_user,
        subject: @import_session,
        action: "import_created",
        request_id: Current.request_id,
        metadata: { row_count: rows.size, file_name: @import_session.file_name }
      )
    end

    redirect_to project_import_session_path(@project, @import_session), notice: "Import created. Source terms are ready for review."
  rescue ActiveRecord::RecordInvalid, CSV::MalformedCSVError => e
    @import_session ||= @project.import_sessions.new(file_name: "source_terms.csv")
    @import_batch = @import_session
    @import_session.errors.add(:base, e.message)
    render :new, status: :unprocessable_entity
  end

  def confirm
    authorize_project!(@project, :import)
    ImportSourceFileJob.perform_later(@import_session.id)
    @import_session.update!(state: "queued")
    redirect_to project_import_session_path(@project, @import_session), notice: "Import queued."
  end

  def show
    authorize_project!(@project, :view)
    @import_batch = @import_session
    @source_terms = @import_session.source_terms.ordered.limit(100)
    render "imports/show"
  end

  private
    def set_project
      @project = Project.find(params[:project_id])
    end

    def set_import_session
      @import_session = @project.import_sessions.find(params[:id])
    end

    def import_params
      params.require(:import_session).permit(
        :file_name, :filename, :source_vocabulary, :file_content_type, :file_size, :rows_text, :source_file,
        column_mapping: {}, detected_columns: []
      )
    end

    def attach_source_file
      file = import_params[:source_file]
      return unless file.present?

      @import_session.file_name = file.original_filename
      @import_session.file_content_type = file.content_type
      @import_session.file_size = file.size
      @import_session.source_file.attach(file)
    end

    def parsed_rows(import_session)
      content = if import_session.rows_text.present?
                  import_session.rows_text
      elsif import_session.source_file.attached?
                  import_session.source_file.download
      else
                  ""
      end
      parse_tabular_content(content, import_session.file_name)
    end

    def parse_tabular_content(content, file_name)
      if file_name.to_s.downcase.end_with?(".xlsx")
        parse_xlsx(content)
      else
        parse_csv(content, col_sep: file_name.to_s.downcase.end_with?(".tsv") ? "\t" : ",")
      end
    end

    def parse_csv(content, col_sep:)
      csv = CSV.parse(content.to_s, headers: true, col_sep: col_sep)
      csv.each_with_index.filter_map do |row, index|
        row_to_source(row.to_h, index + 1)
      end
    end

    def parse_xlsx(content)
      tempfile = Tempfile.new(["source-terms", ".xlsx"], binmode: true)
      tempfile.write(content)
      tempfile.rewind
      sheet = Roo::Spreadsheet.open(tempfile.path).sheet(0)
      headers = sheet.row(1).map(&:to_s)
      (2..sheet.last_row).filter_map do |row_number|
        row_to_source(headers.zip(sheet.row(row_number)).to_h, row_number - 1)
      end
    ensure
      tempfile&.close!
    end

    def row_to_source(row, row_number)
      source_name = row["source_name"].to_s.strip.presence || row["description"].to_s.strip
      return if source_name.blank?

      {
        source_code: row["source_code"].to_s.strip.presence || row["code"].to_s.strip.presence || "ROW_#{row_number}",
        source_name: source_name,
        source_frequency: row["source_frequency"].to_i,
        source_domain_hint: row["domain_hint"].to_s.strip.presence,
        source_row_number: row_number,
        raw_row: row.compact
      }
    end
end
