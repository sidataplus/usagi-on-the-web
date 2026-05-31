require "test_helper"

class EngineClientsTest < ActiveSupport::TestCase
  class FixtureTransport
    attr_reader :requests

    def initialize(payloads)
      @payloads = payloads
      @requests = []
    end

    def post_json(path, payload)
      @requests << [path, payload]
      @payloads.fetch(path)
    end
  end

  test "search client returns deterministic hybrid concept results" do
    results = EngineClients::SearchClient.new.search_concepts(
      q: "tramadol",
      filters: { domain_id: ["Drug"] },
      limit: 3
    )

    assert results.any?
    assert results.all? { |result| result.domain_id == "Drug" }
    assert_equal "hybrid_rrf", results.first.method
    assert_equal 1, results.first.rank
  end

  test "mapper client returns thirawat-style drug candidates" do
    candidates = EngineClients::MapperClient.new.drug_candidates(
      source_name: "tramadol hcl 50mg cap",
      source_code: "SRC_TRAMADOL",
      limit: 2
    )

    assert candidates.any?
    assert_equal "thirawat_tachiom_bimaxsim_tiebreak", candidates.first.method
    assert_equal 1, candidates.first.rank
  end

  test "catalog client resolves one concept" do
    concept = EngineClients::CatalogClient.new.concept(40162522)

    assert_equal 40162522, concept.concept_id
    assert_equal "Drug", concept.domain_id
  end

  test "base client error preserves engine envelope fields" do
    error = EngineClients::BaseClient::Error.from_envelope(
      {
        "error" => {
          "code" => "INDEX_NOT_READY",
          "message" => "SapBERT index is not built",
          "details" => { "required_artifact" => "sapbert_cls.usearch" },
          "request_id" => "req_test"
        }
      },
      status: 503
    )

    assert_equal "INDEX_NOT_READY", error.code
    assert_equal "SapBERT index is not built", error.message
    assert_equal({ "required_artifact" => "sapbert_cls.usearch" }, error.details)
    assert_equal "req_test", error.request_id
    assert_equal 503, error.status
  end

  test "legacy UsagiApi client delegates through engine clients" do
    result = UsagiApi::Client.search_concepts(query: "diabetes", domain: "Condition", limit: 1).first

    assert_equal "Condition", result.domain_id
    assert_equal "hybrid_rrf", result.method
  end

  test "search client parses published contract fixture shape" do
    response = JSON.parse(File.read(Rails.root.join("../../fixtures/contracts/search_concepts.hybrid_rrf.response.json")))
    transport = FixtureTransport.new("/search/concepts" => response)

    results = EngineClients::SearchClient.new(transport: transport).search_concepts(
      q: "tramadol 50 mg capsule",
      filters: { domain_id: ["Drug"] },
      limit: 20
    )

    assert_equal 40162522, results.first.concept_id
    assert_equal "hybrid_rrf", results.first.method
    assert_equal 12.83, results.first.scores["tantivy"]
    assert_equal "local-catalog-standard-v1", results.first.provenance["catalog_artifact_id"]
    assert_equal "/search/concepts", transport.requests.first.first
  end
end
