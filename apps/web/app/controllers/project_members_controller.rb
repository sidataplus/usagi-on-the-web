class ProjectMembersController < ApplicationController
  include ProjectAuthorization

  before_action :set_project
  before_action -> { authorize_project!(@project, :manage) }

  def create
    user = User.find_by(email: params[:email].to_s.strip.downcase)

    if user.nil?
      redirect_to manage_project_path(@project), alert: "No user found with that email."
    elsif @project.project_members.exists?(user: user)
      redirect_to manage_project_path(@project), alert: "#{user.display_name} is already a member."
    else
      member = @project.project_members.create!(user: user, role: member_role)
      audit_member("member_added", member.user, role: member.role)
      redirect_to manage_project_path(@project), notice: "Added #{user.display_name} as #{member.role_label.downcase}."
    end
  end

  def update
    member = @project.project_members.find(params[:id])

    if member.owner?
      redirect_to manage_project_path(@project), alert: "The owner's role can't be changed."
    elsif member.update(role: member_role)
      audit_member("member_role_changed", member.user, role: member.role)
      redirect_to manage_project_path(@project), notice: "Updated #{member.user.display_name} to #{member.role_label.downcase}."
    else
      redirect_to manage_project_path(@project), alert: "Could not update role."
    end
  end

  def destroy
    member = @project.project_members.find(params[:id])

    if member.owner?
      redirect_to manage_project_path(@project), alert: "The project owner can't be removed."
    else
      user = member.user
      member.destroy
      audit_member("member_removed", user)
      redirect_to manage_project_path(@project), notice: "Removed #{user.display_name}."
    end
  end

  private
    def set_project
      @project = Project.find(params[:project_id])
    end

    def member_role
      ProjectMember::ROLES.include?(params[:role]) ? params[:role] : "reviewer"
    end

    def audit_member(action, user, role: nil)
      @project.audit_events.create!(
        user: current_user,
        subject: @project,
        action: action,
        request_id: Current.request_id,
        metadata: { member_user_id: user.id, role: role }.compact
      )
    end
end
