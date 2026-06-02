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
    @import_session = @project.import_sessions.new(file_name: "source_terms.csv", source_vocabulary: @project.source_vocabulary)
    @import_batch = @import_session
    render "imports/new"
  end

  def preview
    authorize_project!(@project, :import)
    @import_session = @project.import_sessions.new(import_params.except(:source_file))
    @import_batch = @import_session
    @import_session.created_by = current_user
    attach_source_file

    rows = parsed_rows(@import_session)
    detected_columns = rows.first&.fetch(:raw_row, {})&.keys || []
    column_mapping = Imports::ColumnDetector.detect(detected_columns)
    mapped_rows = parsed_rows(@import_session, column_mapping: column_mapping)

    @import_session.state = "previewed"
    @import_session.detected_columns = detected_columns
    @import_session.column_mapping = column_mapping
    @import_session.summary = {
      rows_seen: mapped_rows.size,
      preview_rows: [mapped_rows.size, 20].min
    }
    @import_session.save!

    @preview_rows = mapped_rows.first(20)
    render "imports/preview"
  rescue ActiveRecord::RecordInvalid, CSV::MalformedCSVError => e
    @import_session ||= @project.import_sessions.new(file_name: "source_terms.csv")
    @import_batch = @import_session
    @import_session.errors.add(:base, e.message)
    render "imports/new", status: :unprocessable_entity
  end

  def create
    authorize_project!(@project, :import)
    @import_session = @project.import_sessions.new(import_params.except(:source_file))
    @import_batch = @import_session
    @import_session.created_by = current_user
    attach_source_file
    rows = parsed_rows(@import_session)
    detected_columns = rows.first&.fetch(:raw_row, {})&.keys || []
    column_mapping = Imports::ColumnDetector.detect(detected_columns)
    rows = parsed_rows(@import_session, column_mapping: column_mapping)

    ActiveRecord::Base.transaction do
      @import_session.detected_columns = detected_columns
      @import_session.column_mapping = column_mapping
      @import_session.save!
      run_import!(@import_session, rows)
    end

    redirect_to project_import_session_path(@project, @import_session), notice: "Import created. Source terms are ready for review."
  rescue ActiveRecord::RecordInvalid, CSV::MalformedCSVError => e
    @import_session ||= @project.import_sessions.new(file_name: "source_terms.csv")
    @import_batch = @import_session
    @import_session.errors.add(:base, e.message)
    render "imports/new", status: :unprocessable_entity
  end

  def confirm
    authorize_project!(@project, :import)
    column_mapping = submitted_column_mapping.presence || @import_session.column_mapping
    rows = parsed_rows(@import_session, column_mapping: column_mapping)

    ActiveRecord::Base.transaction do
      @import_session.column_mapping = column_mapping
      @import_session.detected_columns = rows.first&.fetch(:raw_row, {})&.keys || @import_session.detected_columns
      run_import!(@import_session, rows)
    end

    redirect_to project_import_session_path(@project, @import_session), notice: "Import confirmed. Source terms are ready for review."
  end

  def show
    authorize_project!(@project, :view)
    @import_batch = @import_session
    @source_terms = @import_session.source_terms.ordered.limit(100)
    @import_mapping_count = @import_session.source_terms.joins(:mapping).count
    @import_candidate_count = @project.mapping_candidates.joins(:source_term)
                                      .where(source_term: { import_session_id: @import_session.id }).count
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

    def submitted_column_mapping
      params.fetch(:import_session, ActionController::Parameters.new)
            .permit(column_mapping: {})
            .fetch(:column_mapping, {})
    end

    def attach_source_file
      file = import_params[:source_file]
      return unless file.present?

      @import_session.file_name = file.original_filename
      @import_session.file_content_type = file.content_type
      @import_session.file_size = file.size
      @import_session.source_file.attach(file)
    end

    def parsed_rows(import_session, column_mapping: {})
      content = if import_session.rows_text.present?
                  import_session.rows_text
      elsif import_session.source_file.attached?
                  import_session.source_file.download
      else
                  ""
      end
      parse_tabular_content(content, import_session.file_name, column_mapping: column_mapping)
    end

    def parse_tabular_content(content, file_name, column_mapping:)
      if file_name.to_s.downcase.end_with?(".xlsx")
        parse_xlsx(content, column_mapping: column_mapping)
      else
        parse_csv(content, col_sep: file_name.to_s.downcase.end_with?(".tsv") ? "\t" : ",", column_mapping: column_mapping)
      end
    end

    def parse_csv(content, col_sep:, column_mapping:)
      csv = CSV.parse(content.to_s, headers: true, col_sep: col_sep)
      csv.each_with_index.filter_map do |row, index|
        row_to_source(row.to_h, index + 1, column_mapping: column_mapping)
      end
    end

    def parse_xlsx(content, column_mapping:)
      tempfile = Tempfile.new(["source-terms", ".xlsx"], binmode: true)
      tempfile.write(content)
      tempfile.rewind
      sheet = Roo::Spreadsheet.open(tempfile.path).sheet(0)
      headers = sheet.row(1).map(&:to_s)
      (2..sheet.last_row).filter_map do |row_number|
        row_to_source(headers.zip(sheet.row(row_number)).to_h, row_number - 1, column_mapping: column_mapping)
      end
    ensure
      tempfile&.close!
    end

    def row_to_source(row, row_number, column_mapping:)
      source_name = mapped_value(row, column_mapping, "source_name", %w[source_name description]).to_s.strip
      source_frequency = mapped_value(row, column_mapping, "source_frequency", %w[source_frequency]).to_i

      {
        source_code: mapped_value(row, column_mapping, "source_code", %w[source_code code]).to_s.strip.presence,
        source_name: source_name,
        source_frequency: source_frequency,
        source_domain_hint: mapped_value(row, column_mapping, "source_domain_hint", %w[source_domain_hint domain_hint]).to_s.strip.presence,
        source_row_number: row_number,
        raw_row: row.compact
      }
    end

    def mapped_value(row, column_mapping, logical_name, fallback_headers)
      header = column_mapping[logical_name].presence
      candidates = [header, *fallback_headers].compact
      matched_key = candidates.lazy.filter_map { |candidate| row.keys.find { |key| key.to_s.casecmp?(candidate.to_s) } }.first
      matched_key ? row[matched_key] : nil
    end

    def run_import!(import_session, rows)
      import_session.state = "running"
      import_session.save!

      import_result = Imports::SourceTermImporter.new(
        project: @project,
        import_session: import_session
      ).import(rows)

      import_session.state = import_result.rows_failed.positive? ? "succeeded_with_errors" : "succeeded"
      import_session.summary = {
        rows_seen: import_result.rows_seen,
        rows_imported: import_result.rows_imported,
        rows_failed: import_result.rows_failed
      }
      import_session.error = import_result.errors.any? ? { row_errors: import_result.errors } : {}
      import_session.finished_at = Time.current
      import_session.save!

      @project.audit_events.create!(
        user: current_user,
        subject: import_session,
        action: "import_created",
        request_id: Current.request_id,
        metadata: {
          row_count: import_result.rows_imported,
          rows_failed: import_result.rows_failed,
          file_name: import_session.file_name
        }
      )
    end
end
