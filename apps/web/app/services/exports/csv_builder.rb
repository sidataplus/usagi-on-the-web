require "csv"

module Exports
  class CsvBuilder
    def initialize(project, format:)
      @project = project
      @format = format
    end

    def to_csv
      case format
      when "source_to_concept_map_csv" then source_to_concept_map_csv
      when "candidate_csv" then candidate_csv
      when "audit_csv" then audit_csv
      else review_csv
      end
    end

    private
      attr_reader :project, :format

      def review_csv
        CSV.generate(headers: true) do |csv|
          csv << %w[source_code source_name source_frequency concept_id concept_name vocabulary_id status equivalence match_score]
          project.mappings.includes(:source_term).ordered.each do |mapping|
            csv << [
              mapping.source_code, mapping.source_name, mapping.source_frequency,
              mapping.target_concept_id, mapping.target_concept_name, mapping.target_vocabulary_id,
              mapping.status_label, mapping.equivalence, mapping.match_score
            ]
          end
        end
      end

      def source_to_concept_map_csv
        CSV.generate(headers: true) do |csv|
          csv << %w[source_code source_name target_concept_id target_concept_name target_vocabulary_id equivalence]
          project.mappings.includes(:source_term).where(mapping_status: "APPROVED").ordered.each do |mapping|
            csv << [
              mapping.source_code, mapping.source_name, mapping.target_concept_id,
              mapping.target_concept_name, mapping.target_vocabulary_id, mapping.equivalence
            ]
          end
        end
      end

      def candidate_csv
        CSV.generate(headers: true) do |csv|
          csv << %w[source_code source_name rank concept_id concept_name method score provenance]
          project.mapping_candidates.includes(:mapping, :source_term).order(:source_term_id, :rank).each do |candidate|
            csv << [
              candidate.source_term.source_code, candidate.source_term.source_name, candidate.rank,
              candidate.concept_id, candidate.concept_name, candidate.method, candidate.score,
              candidate.provenance.to_json
            ]
          end
        end
      end

      def audit_csv
        CSV.generate(headers: true) do |csv|
          csv << %w[created_at action subject_type subject_id user_id request_id metadata]
          project.audit_events.recent_first.each do |event|
            csv << [event.created_at.iso8601, event.action, event.subject_type, event.subject_id, event.user_id, event.request_id, event.metadata.to_json]
          end
        end
      end
  end
end
