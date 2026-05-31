class PagesController < ApplicationController
  skip_before_action :authenticate_user!, only: %i[privacy terms not_found]

  def privacy
  end

  def terms
  end

  def not_found
    render status: :not_found
  end
end
