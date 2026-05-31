module ProjectAuthorization
  extend ActiveSupport::Concern

  private
    def authorize_project!(project, action)
      return true if current_user&.admin?

      member = project.project_members.find_by(user: current_user)
      allowed = case action
      when :view then member.present?
      when :import then member&.can_import?
      when :suggest then member&.can_run_suggestions?
      when :review then member&.can_review?
      when :export then member&.can_export?
      when :manage then member&.can_manage_members?
      else false
      end
      raise ActiveRecord::RecordNotFound unless allowed

      true
    end
end
