# Shared review-cockpit helpers: where a mapping sits in the review order, the
# locals the cockpit partial needs, and the Turbo Streams that refresh the table
# row and either advance the open cockpit to the next mapping or close it.
module MappingCockpit
  extend ActiveSupport::Concern

  private
    def navigation_for(mapping)
      ordered = mapping.project.mappings.ordered.pluck(:id)
      index = ordered.index(mapping.id)
      return { position: nil, total: ordered.size, prev_id: nil, next_id: nil } unless index

      { position: index + 1, total: ordered.size,
        prev_id: index.positive? ? ordered[index - 1] : nil,
        next_id: index < ordered.size - 1 ? ordered[index + 1] : nil }
    end

    def next_mapping_id(mapping)
      navigation_for(mapping)[:next_id]
    end

    def cockpit_locals(mapping)
      {
        mapping: mapping,
        siblings: navigation_for(mapping),
        candidates: mapping.persisted_candidates,
        events: mapping.project.audit_events.where(subject: mapping).recent_first,
        comments: mapping.comments.includes(:user).chronological
      }
    end

    # advance_to: the next Mapping to load into the open cockpit, or nil to close it.
    def cockpit_decision_streams(current_mapping, advance_to: nil)
      streams = [
        turbo_stream.replace(
          helpers.dom_id(current_mapping, :row),
          partial: "mappings/row",
          locals: { mapping: current_mapping, highlight: true }
        )
      ]
      streams << if advance_to
        turbo_stream.update("modal", partial: "mappings/cockpit", locals: cockpit_locals(advance_to))
      else
        turbo_stream.update("modal", "")
      end
      streams
    end
end
