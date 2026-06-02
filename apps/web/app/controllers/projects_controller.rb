class ProjectsController < ApplicationController
  include ProjectAuthorization

  before_action :set_project, only: %i[show manage update]

  PER_PAGE = 9

  def index
    @page = [params[:page].to_i, 1].max
    scope = current_user.admin? ? Project.all : current_user.projects
    scope = scope.order(updated_at: :desc)
    @total_pages = [(scope.count.to_f / PER_PAGE).ceil, 1].max
    @projects = scope.offset((@page - 1) * PER_PAGE).limit(PER_PAGE)
  end

  def show
    authorize_project!(@project, :view)
    @recent_mappings = @project.mappings.ordered.limit(5)
    @recent_imports = @project.import_sessions.recent_first.limit(5)
    @recent_engine_jobs = @project.engine_jobs.recent_first.limit(5)
    @latest_suggestion_job = @project.engine_jobs.where(kind: suggestion_job_kinds).recent_first.first
    @latest_export = @project.exports.recent_first.first
    @engine_readiness = engine_readiness(@project)
  end

  def new
    @project = Project.new(mapping_domain: "Drug")
  end

  def create
    @project = Project.new(project_params)
    @project.created_by = current_user

    if @project.save
      redirect_to project_path(@project), notice: "Project created. Next, import your source terms."
    else
      render :new, status: :unprocessable_entity
    end
  end

  def manage
    authorize_project!(@project, :manage)
    @memberships = @project.project_members.includes(:user)
  end

  def update
    authorize_project!(@project, :manage)
    if @project.update(project_params)
      redirect_to manage_project_path(@project), notice: "Project settings saved."
    else
      @memberships = @project.project_members.includes(:user)
      render :manage, status: :unprocessable_entity
    end
  end

  private
    def set_project
      @project = Project.find(params[:id])
    end

    def project_params
      params.require(:project).permit(
        :name, :description, :source_vocabulary, :vocabulary_version, :status, :mapping_domain,
        :target_vocabularies, target_domain_ids: [], target_vocabulary_ids: []
      )
    end

    def suggestion_job_kinds
      @project.drug_domain? ? ["mapper_drugs_batch"] : ["hybrid_search_batch"]
    end

    def engine_readiness(project)
      catalog_status = engine_status { EngineClients::CatalogClient.new.status }
      suggestion_status = if project.drug_domain?
        engine_status { EngineClients::MapperClient.new.status }
      else
        engine_status { EngineClients::SearchClient.new.status }
      end

      {
        catalog: catalog_status,
        suggestion: suggestion_status,
        suggestion_label: project.drug_domain? ? "Drug mapper" : "Hybrid search",
        ready: engine_ready?(catalog_status) && engine_ready?(suggestion_status)
      }
    end

    def engine_status
      yield
    rescue EngineClients::BaseClient::Error => e
      {
        "status" => "unavailable",
        "error" => {
          "code" => e.code,
          "message" => e.message,
          "request_id" => e.request_id
        }
      }
    end

    def engine_ready?(payload)
      payload["status"].to_s == "ready"
    end
end
