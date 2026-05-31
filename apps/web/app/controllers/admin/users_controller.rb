class Admin::UsersController < ApplicationController
  before_action :require_admin

  def index
    @users = User.order(:name)
  end

  def update
    @user = User.find(params[:id])
    @user.admin = ActiveModel::Type::Boolean.new.cast(params.dig(:user, :admin))
    if @user.save
      redirect_to admin_users_path, notice: "Updated #{@user.display_name}."
    else
      redirect_to admin_users_path, alert: "Could not update user."
    end
  end
end
