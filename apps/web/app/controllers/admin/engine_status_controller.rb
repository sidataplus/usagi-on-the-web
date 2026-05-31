class Admin::EngineStatusController < ApplicationController
  before_action :require_admin

  def show
    @catalog_status = status_for { EngineClients::CatalogClient.new.status }
    @search_status = status_for { EngineClients::SearchClient.new.status }
    @mapper_status = status_for { EngineClients::MapperClient.new.status }
  end

  private
    def status_for
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
end
