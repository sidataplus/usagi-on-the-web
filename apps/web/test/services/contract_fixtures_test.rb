require "test_helper"

class ContractFixturesTest < ActiveSupport::TestCase
  FIXTURE_ROOT = Rails.root.join("..", "..", "fixtures", "contracts")

  test "search concept response fixture parses into concept results" do
    payload = JSON.parse(FIXTURE_ROOT.join("search_concepts.hybrid_rrf.response.json").read)
    results = payload.fetch("results").map do |result|
      EngineClients::SearchClient.new.send(:concept_result_from_payload, result, provenance: payload.fetch("provenance"))
    end

    assert_equal 123456, results.first.concept_id
    assert_equal "hybrid_rrf", results.first.method
    assert_equal "local-catalog-standard-v1", results.first.provenance.fetch("catalog_artifact_id")
  end

  test "mapper candidate fixture preserves scores features and provenance" do
    payload = JSON.parse(FIXTURE_ROOT.join("mapper_drugs_query.response.json").read)
    result = EngineClients::MapperClient.new.send(
      :concept_result_from_payload,
      payload.fetch("candidates").first,
      provenance: payload.fetch("provenance")
    )

    assert_equal "thirawat_tachiom_bimaxsim_tiebreak", result.method
    assert_equal 0.94, result.score
    assert_equal true, result.features.fetch("strength_exact")
    assert_equal "sidataplus/THIRAWAT-SapBERT", result.provenance.fetch("model_artifact_id")
  end

  test "error fixture parses into engine client error" do
    payload = JSON.parse(FIXTURE_ROOT.join("error.index_not_ready.json").read)
    error = EngineClients::BaseClient::Error.from_envelope(payload, status: 503)

    assert_equal "INDEX_NOT_READY", error.code
    assert_equal "SapBERT index is not built", error.message
    assert_equal "req_contract", error.request_id
  end
end
