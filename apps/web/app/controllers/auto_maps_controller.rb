class AutoMapsController < ApplicationController
  include ProjectAuthorization

  before_action :set_project

  def create
    authorize_project!(@project, :suggest)
    if @project.drug_domain?
      StartAutoMapJob.perform_later(@project.id)
    else
      RunHybridSearchJob.perform_later(@project.id)
    end
    @project.audit_events.create!(
      user: current_user,
      subject: @project,
      action: "auto_map_requested",
      request_id: Current.request_id,
      metadata: { mapping_domain: @project.mapping_domain }
    )
    redirect_to project_engine_jobs_path(@project), notice: "Auto-suggest started."
  end

  private
    def set_project
      @project = Project.find(params[:project_id])
    end
end
