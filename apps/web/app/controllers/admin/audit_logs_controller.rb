class Admin::AuditLogsController < ApplicationController
  before_action :require_admin

  def index
    @events = AuditEvent.includes(:user, :project).recent_first.limit(100)
  end
end
