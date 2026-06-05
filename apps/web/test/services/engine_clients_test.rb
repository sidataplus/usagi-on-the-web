require "test_helper"

class EngineClientsTest < ActiveSupport::TestCase
  class RecordingLogger
    attr_reader :infos, :warns

    def initialize
      @infos = []
      @warns = []
    end

    def info(payload)
      @infos << payload
    end

    def warn(payload)
      @warns << payload
    end
  end

  class FixedSigner
    def headers(method:, path_with_query:, body:, request_id:)
      {
        "X-Request-Id" => request_id,
        "X-Test-Method" => method,
        "X-Test-Path" => path_with_query,
        "X-Test-Body-SHA" => Digest::SHA256.hexdigest(body.to_s)
      }
    end
  end

  class RecordingHttpClient
    attr_reader :requests

    def initialize(outcomes)
      @outcomes = outcomes
      @requests = []
    end

    def start(_host, _port, **)
      connection = Connection.new(@outcomes, @requests)
      yield connection
    end

    class Connection
      def initialize(outcomes, requests)
        @outcomes = outcomes
        @requests = requests
      end

      def request(request)
        @requests << request
        outcome = @outcomes.shift
        raise outcome if outcome.is_a?(Exception)

        outcome
      end
    end
  end

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

  class FailingJsonlResultsTransport
    def get_json(path)
      raise "unexpected path #{path}" unless path == "/jobs/job_live/results"

      { "job_id" => "job_live", "state" => "succeeded", "artifact" => { "content_type" => "application/jsonl" } }
    end

    def get_jsonl(path)
      raise "unexpected path #{path}" unless path == "/jobs/job_live/results"

      raise EngineClients::BaseClient::Error.new(
        code: "RESULT_STREAM_UNAVAILABLE",
        message: "Result artifact is unavailable",
        request_id: "req_results"
      )
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

  test "http transport retries transient GET failures and logs the completed request" do
    logger = RecordingLogger.new
    http = RecordingHttpClient.new([
      Errno::ECONNRESET.new("reset by peer"),
      http_response(Net::HTTPOK, { "status" => "ready" })
    ])
    transport = EngineClients::HttpTransport.new(
      base_url: "http://search-api:8789",
      signer: FixedSigner.new,
      logger: logger,
      http_client: http,
      max_get_retries: 1
    )

    payload = transport.get_json("/search/status")

    assert_equal "ready", payload.fetch("status")
    assert_equal 2, http.requests.size
    assert_equal "GET", http.requests.first.method
    assert_equal "/search/status", http.requests.first["X-Test-Path"]
    assert_equal 1, logger.warns.size
    assert_equal "engine.request.retry", logger.warns.first.fetch(:event)
    assert_equal "engine.request", logger.infos.last.fetch(:event)
    assert_equal "GET", logger.infos.last.fetch(:method)
    assert_equal "/search/status", logger.infos.last.fetch(:path)
    assert_equal 200, logger.infos.last.fetch(:status)
  end

  test "jobs client propagates result JSONL failures instead of returning empty items" do
    error = assert_raises(EngineClients::BaseClient::Error) do
      EngineClients::JobsClient.new(transport: FailingJsonlResultsTransport.new).results("job_live")
    end

    assert_equal "RESULT_STREAM_UNAVAILABLE", error.code
    assert_equal "req_results", error.request_id
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

  private
    def http_response(response_class, payload, status_message: "OK")
      response = response_class.new("1.1", "200", status_message)
      response.instance_variable_set(:@read, true)
      response.body = JSON.generate(payload)
      response
    end
end
