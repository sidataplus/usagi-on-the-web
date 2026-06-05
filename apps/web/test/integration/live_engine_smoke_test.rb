require "test_helper"

class LiveEngineSmokeTest < ActiveSupport::TestCase
  ProjectDouble = Struct.new(:id, :settings, keyword_init: true)

  test "live engine status endpoints respond when explicitly enabled" do
    skip_unless_live_engine!

    catalog_status = catalog_client.status
    search_status = search_client.status
    mapper_status = mapper_client.status

    assert_includes %w[ready not_configured degraded], catalog_status.fetch("status")
    assert_includes %w[ready not_configured degraded], search_status.fetch("status")
    assert_includes %w[ready not_configured degraded], mapper_status.fetch("status")
  end

  test "live search endpoint returns candidate DTOs when configured" do
    skip_unless_live_engine!

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

  test "live search batch preserves response ids and provenance" do
    skip_unless_live_engine!
    skip "search-api is not ready" unless search_client.status.fetch("status") == "ready"

    response = search_client.batch(
      filters: { domain_id: ["Drug"] },
      hybrid: { lexical_top_k: 10, sapbert_top_k: 10 },
      limit_per_item: 1,
      items: [
        {
          id: "src_live_tramadol_50",
          source_code: "SRC_TRAMADOL_50_CAP",
          q: "tramadol 50 mg capsule",
          filters: { vocabulary_id: ["RxNorm"] }
        }
      ]
    )

    item = response.fetch("items").first
    assert_equal "src_live_tramadol_50", item.fetch("id")
    assert_equal 100, item.fetch("results").first.fetch("concept").fetch("concept_id")
    assert_equal "local-hybrid-rrf-v1", response.fetch("provenance").fetch("index_artifact_id")
  end

  test "live mapper endpoint returns candidate DTOs when configured" do
    skip_unless_live_engine!

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

  test "live mapper batch job can be polled and streamed through Rails clients" do
    skip_unless_live_engine!
    skip "mapper-api is not ready" unless mapper_client.status.fetch("status") == "ready"

    project = ProjectDouble.new(
      id: "proj_live_#{SecureRandom.hex(6)}",
      settings: { "candidate_limit" => 1 }
    )
    item_id = "live-tramadol-#{SecureRandom.hex(6)}"
    create_response = mapper_client.start_drug_batch_job(
      project: project,
      idempotency_key: "rails-live-mapper-#{SecureRandom.hex(12)}",
      items: [
        {
          id: item_id,
          source_name: "tramadol hydrochloride 50 mg capsule",
          source_code: "SRC_TRAMADOL_50_CAP"
        }
      ]
    )

    job_id = create_response.fetch("job_id")
    status = wait_for_live_job(job_id)
    assert_equal "succeeded", status.fetch("state")
    assert_equal 1, status.fetch("processed")
    assert_equal 0, status.fetch("failed")

    results = jobs_client.results(job_id)
    result_item = results.fetch("items").find { |item| item.fetch("id") == item_id }
    assert_not_nil result_item
    assert_equal 40162522, result_item.fetch("candidates").first.fetch("concept").fetch("concept_id")
  end

  private
    def skip_unless_live_engine!
      skip "set USAGI_LIVE_ENGINE=1 to run against local usagi-api" unless ENV["USAGI_LIVE_ENGINE"] == "1"
    end

    def catalog_client
      EngineClients::CatalogClient.new(
        transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("CATALOG_API_URL", "http://127.0.0.1:8788"))
      )
    end

    def search_client
      EngineClients::SearchClient.new(
        transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("SEARCH_API_URL", "http://127.0.0.1:8789"))
      )
    end

    def mapper_client
      EngineClients::MapperClient.new(
        transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("MAPPER_API_URL", "http://127.0.0.1:8790"))
      )
    end

    def jobs_client
      EngineClients::JobsClient.new(
        transport: EngineClients::HttpTransport.new(base_url: ENV.fetch("JOBS_API_URL", ENV.fetch("MAPPER_API_URL", "http://127.0.0.1:8790")))
      )
    end

    def wait_for_live_job(job_id)
      deadline = 30.seconds.from_now
      loop do
        status = jobs_client.status(job_id)
        return status if status.fetch("state") == "succeeded"
        flunk "live job #{job_id} ended in #{status.fetch("state")}: #{status}" if status.fetch("state").in?(%w[failed cancelled succeeded_with_errors])
        flunk "timed out waiting for live job #{job_id}" if Time.current > deadline

        sleep 0.5
      end
    end
end
