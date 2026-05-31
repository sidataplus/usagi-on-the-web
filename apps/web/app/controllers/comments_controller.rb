class CommentsController < ApplicationController
  include ProjectAuthorization

  before_action :set_mapping

  def index
    authorize_project!(@mapping.project, :view)
    @comments = @mapping.comments.includes(:user).chronological
    render partial: "mappings/comments", locals: { mapping: @mapping, comments: @comments }
  end

  def create
    authorize_project!(@mapping.project, :review)
    @comment = @mapping.comments.build(comment_params.merge(project: @mapping.project, user: current_user))
    if @comment.save
      @mapping.project.audit_events.create!(
        user: current_user,
        subject: @mapping,
        action: "commented",
        request_id: Current.request_id,
        metadata: { comment_id: @comment.id }
      )
      respond_to do |format|
        format.turbo_stream { render "mappings/comments/create" }
        format.html { redirect_to mapping_path(@mapping) }
      end
    else
      render turbo_stream: turbo_stream.replace(
        "comment_form_#{@mapping.id}",
        partial: "mappings/comment_form",
        locals: { mapping: @mapping, comment: @comment }
      ), status: :unprocessable_entity
    end
  end

  private
    def set_mapping
      @mapping = Mapping.find(params[:mapping_id])
    end

    def comment_params
      params.require(:comment).permit(:content, :parent_id)
    rescue ActionController::ParameterMissing
      params.require(:mapping_comment).permit(:content, :parent_id)
    end
end
