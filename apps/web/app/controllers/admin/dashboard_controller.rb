class Admin::DashboardController < ApplicationController
  before_action :require_admin

  def show
    @user_count = User.count
    @project_count = Project.count
    @mapping_count = Mapping.count
    @recent_users = User.order(created_at: :desc).limit(5)
  end
end
