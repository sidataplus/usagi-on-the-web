class GuestAccessController < ApplicationController
  # Placeholder for read-only guest pass access (/guest/:pass_id). Real pass
  # resolution + read-only project view lands with the auth/permissions work.
  def show
    @pass_id = params[:pass_id]
  end
end
