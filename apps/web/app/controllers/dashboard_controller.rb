class DashboardController < ApplicationController
  before_action :require_admin

  def show
    @project_count = Project.count
    @mapping_count = Mapping.count
    @status_counts = Mapping.group(:mapping_status).count
    @approved = @status_counts["APPROVED"].to_i
    @approval_rate = @mapping_count.zero? ? 0 : ((@approved.to_f / @mapping_count) * 100).round
    @domain_breakdown = Mapping.group(:target_domain_id).count.compact.sort_by { |_, value| -value }
    @top_projects = Project.all.sort_by { |project| -project.total_mappings }.first(5)
    @reviewers = User.all.map { |user| [user, Mapping.where(reviewed_by: user).count] }.sort_by { |_, count| -count }
  end
end
