class MappingEvent < AuditEvent
  self.table_name = "audit_events"
  prefixed_id "aud"

  def author_name
    user&.display_name || "System"
  end

  def action_label
    action.to_s.humanize
  end

  def from_value
    metadata["from"]
  end

  def to_value
    metadata["to"]
  end
end
