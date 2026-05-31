class ImportBatch < ImportSession
  self.table_name = "import_sessions"
  prefixed_id "imp"

  def uploaded_by
    created_by
  end

  def uploaded_by=(user)
    self.created_by = user
  end

  def filename
    file_name
  end

  def filename=(value)
    self.file_name = value
  end

  def source_vocabulary
    project&.source_vocabulary
  end

  def status
    state
  end

  def status=(value)
    self.state = value.to_s == "imported" ? "succeeded" : value
  end

  def imported_at
    finished_at
  end

  def imported_at=(value)
    self.finished_at = value
  end

  def metadata
    summary
  end

  def error_summary
    error
  end
end
