class EngineJobsController < ApplicationController
  include ProjectAuthorization

  before_action :set_project
  before_action :set_engine_job, only: :show

  def index
    authorize_project!(@project, :view)
    @engine_jobs = @project.engine_jobs.recent_first
  end

  def show
    authorize_project!(@project, :view)
    @engine_status = EngineClients::JobsClient.new.status(@engine_job)
  end

  private
    def set_project
      @project = Project.find(params[:project_id])
    end

    def set_engine_job
      @engine_job = @project.engine_jobs.find(params[:id])
    end
end
