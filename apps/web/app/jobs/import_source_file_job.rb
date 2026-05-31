class ImportSourceFileJob < ApplicationJob
  queue_as :default

  def perform(import_session_id)
    import_session = ImportSession.find(import_session_id)
    import_session.update!(state: "succeeded", finished_at: Time.current)
  end
end
