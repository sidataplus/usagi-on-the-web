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

  test "live search endpoint returns candidate DTOs when configured" do
    skip "set USAGI_LIVE_ENGINE=1 to run against local usagi-api" unless ENV["USAGI_LIVE_ENGINE"] == "1"

    search_client = EngineClients::SearchClient.new(
      transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("SEARCH_API_URL", "http://127.0.0.1:8789"))
    )

    search_status = search_client.status.fetch("status")
    unless search_status == "ready"
      skip "search-api is #{search_status}; live candidate query requires ready search artifacts"
    end

    search_results = search_client.search_concepts(
      q: "tramadol 50 mg capsule",
      filters: { domain_id: ["Drug"] },
      limit: 3
    )

    assert search_results.all? { |result| result.is_a?(UsagiApi::ConceptResult) }
    assert search_results.all? { |result| result.concept_id.present? && result.concept_name.present? }
  end

  test "live mapper endpoint returns candidate DTOs when configured" do
    skip "set USAGI_LIVE_ENGINE=1 to run against local usagi-api" unless ENV["USAGI_LIVE_ENGINE"] == "1"

    mapper_client = EngineClients::MapperClient.new(
      transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("MAPPER_API_URL", "http://127.0.0.1:8790"))
    )

    mapper_status = mapper_client.status.fetch("status")
    unless mapper_status == "ready"
      skip "mapper-api is #{mapper_status}; live drug mapper query requires ready mapper artifacts"
    end

    mapper_results = mapper_client.drug_candidates(
      source_name: "tramadol hydrochloride 50 mg capsule",
      source_code: "SRC_TRAMADOL_50_CAP",
      limit: 3
    )

    assert mapper_results.all? { |result| result.is_a?(UsagiApi::ConceptResult) }
    assert mapper_results.all? { |result| result.concept_id.present? && result.concept_name.present? }
  end
end
