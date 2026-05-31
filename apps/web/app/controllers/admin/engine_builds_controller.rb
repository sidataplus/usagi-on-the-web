class Admin::EngineBuildsController < ApplicationController
  before_action :require_admin

  def create
    redirect_to admin_engine_status_path, alert: "Engine build controls are intentionally deferred for the MVP."
  end
end
