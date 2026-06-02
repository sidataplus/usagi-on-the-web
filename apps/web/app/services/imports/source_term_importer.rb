require "set"

module Imports
  ImportResult = Data.define(:rows_seen, :rows_imported, :rows_failed, :errors)

  class SourceTermImporter
    def initialize(project:, import_session:)
      @project = project
      @import_session = import_session
    end

    def import(rows)
      errors = []
      imported = 0
      seen_codes = Set.new

      Array(rows).each do |row|
        row = row.symbolize_keys
        source_code = row[:source_code].to_s.strip
        row_number = row[:source_row_number].to_i

        if source_code.blank?
          errors << row_error(row_number, "source_code is required")
          next
        end
        if seen_codes.include?(source_code) || project.source_terms.exists?(source_code: source_code)
          errors << row_error(row_number, "duplicate source_code #{source_code}")
          next
        end
        if row[:source_name].to_s.strip.blank?
          errors << row_error(row_number, "source_name is required")
          next
        end

        source_term = import_session.source_terms.create!(
          project: project,
          source_code: source_code,
          source_name: row[:source_name],
          source_frequency: row[:source_frequency].to_i,
          source_domain_hint: row[:source_domain_hint],
          source_vocabulary: import_session.source_vocabulary.presence || project.source_vocabulary,
          source_row_number: row_number,
          raw_row: row[:raw_row] || {}
        )
        source_term.create_mapping!(project: project, mapping_status: "UNCHECKED")
        seen_codes << source_code
        imported += 1
      rescue ActiveRecord::RecordInvalid => e
        errors << row_error(row_number, e.record.errors.full_messages.to_sentence)
      end

      ImportResult.new(
        rows_seen: Array(rows).size,
        rows_imported: imported,
        rows_failed: errors.size,
        errors: errors
      )
    end

    private
      attr_reader :project, :import_session

      def row_error(row_number, message)
        { row_number: row_number, message: message }
      end
  end
end
