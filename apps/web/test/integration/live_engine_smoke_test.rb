require "test_helper"

class LiveEngineSmokeTest < ActiveSupport::TestCase
  test "live engine status endpoints respond when explicitly enabled" do
    skip "set USAGI_LIVE_ENGINE=1 to run against local usagi-api" unless ENV["USAGI_LIVE_ENGINE"] == "1"

    catalog_status = EngineClients::CatalogClient.new(
      transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("CATALOG_API_URL", "http://127.0.0.1:8788"))
    ).status
    search_status = EngineClients::SearchClient.new(
      transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("SEARCH_API_URL", "http://127.0.0.1:8789"))
    ).status
    mapper_status = EngineClients::MapperClient.new(
      transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("MAPPER_API_URL", "http://127.0.0.1:8790"))
    ).status

    assert_includes %w[ready not_configured degraded], catalog_status.fetch("status")
    assert_includes %w[ready not_configured degraded], search_status.fetch("status")
    assert_includes %w[ready not_configured degraded], mapper_status.fetch("status")
  end
end
