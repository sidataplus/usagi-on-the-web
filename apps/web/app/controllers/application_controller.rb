class ApplicationController < ActionController::Base
  allow_browser versions: :modern
  stale_when_importmap_changes

  before_action :set_current_request
  before_action :authenticate_user!

  helper_method :current_user, :signed_in?

  rescue_from EngineClients::BaseClient::Error, with: :handle_engine_error
  rescue_from ActiveRecord::RecordNotFound, with: :handle_not_found
  rescue_from ActiveRecord::StaleObjectError, with: :handle_stale_object

  private
    def set_current_request
      Current.request_id = request.request_id
      Current.user = current_user
    end

    def current_user
      @current_user ||= User.find_by(id: session[:user_id]) if session[:user_id].present?
    end

    def signed_in?
      current_user.present?
    end

    def authenticate_user!
      return if signed_in?

      redirect_to new_session_path, alert: "Sign in to continue."
    end

    def require_admin
      return if current_user&.can_access_admin?

      redirect_to projects_path, alert: "You don't have access to that area."
    end

    def handle_engine_error(error)
      message = [error.message, error.request_id.presence && "request #{error.request_id}"].compact.join(" - ")
      respond_to do |format|
        format.turbo_stream do
          flash.now[:alert] = message
          render turbo_stream: turbo_stream.update("flash", partial: "shared/flash"), status: :service_unavailable
        end
        format.html { redirect_back fallback_location: projects_path, alert: message }
      end
    end

    def handle_not_found
      render "pages/not_found", status: :not_found
    end

    def handle_stale_object
      message = "This mapping changed since you opened it. Reopen it and try again."
      respond_to do |format|
        format.turbo_stream do
          flash.now[:alert] = message
          render turbo_stream: turbo_stream.update("flash", partial: "shared/flash"), status: :conflict
        end
        format.html { redirect_back fallback_location: projects_path, alert: message }
      end
    end
end
