class EngineJobsController < ApplicationController
  include ProjectAuthorization

  before_action :set_project
  before_action :set_engine_job, only: %i[show retry]

  def index
    authorize_project!(@project, :view)
    @engine_jobs = @project.engine_jobs.recent_first
  end

  def show
    authorize_project!(@project, :view)
    @engine_status = EngineClients::JobsClient.new.status(@engine_job)
  end

  def retry
    authorize_project!(@project, :suggest)
    case @engine_job.kind
    when "mapper_drugs_batch"
      StartAutoMapJob.perform_later(@project.id)
    when "hybrid_search_batch"
      RunHybridSearchJob.perform_later(@project.id)
    else
      redirect_to project_engine_job_path(@project, @engine_job), alert: "This engine job cannot be retried."
      return
    end

    @project.audit_events.create!(
      user: current_user,
      subject: @engine_job,
      action: "engine_job_retry_requested",
      request_id: Current.request_id,
      metadata: { kind: @engine_job.kind, previous_state: @engine_job.state }
    )
    redirect_to project_engine_jobs_path(@project), notice: "Engine job retry queued."
  end

  private
    def set_project
      @project = Project.find(params[:project_id])
    end

    def set_engine_job
      @engine_job = @project.engine_jobs.find(params[:id])
    end
end
