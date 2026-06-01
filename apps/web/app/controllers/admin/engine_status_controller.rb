class Admin::EngineStatusController < ApplicationController
  before_action :require_admin

  def show
    @catalog_status = status_for { EngineClients::CatalogClient.new.status }
    @search_status = status_for { EngineClients::SearchClient.new.status }
    @mapper_status = status_for { EngineClients::MapperClient.new.status }
    @engine_services = [
      service_card("Catalog lookup", @catalog_status),
      service_card("Hybrid search", @search_status),
      service_card("Drug mapper", @mapper_status)
    ]
    @mapping_ready = @engine_services.all? { |service| service[:status] == "ready" }
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

    def service_card(label, payload)
      status = payload["status"].to_s.presence || "unknown"
      {
        label: label,
        status: status,
        tone: tone_for(status),
        summary: "#{label} is #{status.tr("_", " ")}",
        artifacts: artifact_lines(payload),
        action_items: action_items(payload),
        payload: payload
      }
    end

    def tone_for(status)
      case status
      when "ready" then "ok"
      when "degraded" then "warn"
      else "bad"
      end
    end

    def artifact_lines(value, prefix = nil)
      case value
      when Hash
        value.flat_map do |key, nested|
          label = [prefix, key].compact.join(".")
          interesting_artifact_key?(key) ? ["#{label}: #{nested}"] : artifact_lines(nested, label)
        end
      when Array
        value.flat_map.with_index { |nested, index| artifact_lines(nested, "#{prefix}[#{index}]") }
      else
        []
      end
    end

    def action_items(value)
      artifact_lines(value).select { |line| line.include?("required_artifact") }
    end

    def interesting_artifact_key?(key)
      %w[artifact_id required_artifact vocabulary_version concept_count].include?(key.to_s)
    end
end
