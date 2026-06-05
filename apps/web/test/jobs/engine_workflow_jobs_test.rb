require "test_helper"

class EngineWorkflowJobsTest < ActiveJob::TestCase
  class MapperBatchTransport
    attr_reader :requests

    def initialize(response = {})
      @response = {
        "job_id" => "api_mapper_batch_1",
        "state" => "queued",
        "status_url" => "/jobs/api_mapper_batch_1",
        "result_url" => "/jobs/api_mapper_batch_1/results"
      }.merge(response)
      @requests = []
    end

    def post_json(path, payload)
      @requests << [path, payload]
      @response
    end
  end

  class JobsTransport
    def initialize(status:, results: nil)
      @status = status
      @results = results || { "items" => [] }
    end

    def get_json(path)
      path.end_with?("/results") ? @results : @status
    end
  end

  # Mirrors the live engine: /results returns an artifact envelope (no inline
  # items), and the per-term results are served as JSONL via get_jsonl.
  class JsonlResultsTransport
    def initialize(envelope:, jsonl:)
      @envelope = envelope
      @jsonl = jsonl
    end

    def get_json(_path)
      @envelope
    end

    def get_jsonl(_path)
      @jsonl
    end
  end

  class FailingJsonlResultsTransport
    def get_json(_path)
      {
        "job_id" => "api_failed_jsonl_results",
        "state" => "succeeded",
        "artifact" => { "content_type" => "application/jsonl", "path" => "data/jobs/results/api_failed_jsonl_results/results.jsonl" }
      }
    end

    def get_jsonl(_path)
      raise EngineClients::BaseClient::Error.new(
        code: "RESULT_STREAM_UNAVAILABLE",
        message: "Result artifact is unavailable",
        request_id: "req_result_stream",
        status: 502
      )
    end
  end

  class FailingSearchTransport
    def search_concepts(**)
      raise EngineClients::BaseClient::Error.new(code: "INDEX_NOT_READY", message: "Search index is not built", request_id: "req_fail")
    end
  end

  class FailingMapperBatchTransport
    def post_json(_path, _payload)
      raise EngineClients::BaseClient::Error.new(code: "ENGINE_UNAVAILABLE", message: "mapper unavailable", request_id: "req_mapper_fail", status: 503)
    end
  end

  class BatchSearchTransport
    attr_reader :requests

    def initialize
      @requests = []
    end

    def post_json(path, payload)
      @requests << [path, payload]
      {
        "items" => payload.fetch(:items).map do |item|
          id = item.fetch(:id)
          if item.fetch(:source_code) == "DX_FAIL"
            {
              "id" => id,
              "state" => "failed",
              "error" => { "code" => "BAD_REQUEST", "message" => "Cannot map source term" }
            }
          else
            {
              "id" => id,
              "state" => "succeeded",
              "results" => [
                {
                  "rank" => 1,
                  "method" => "hybrid_rrf",
                  "concept" => {
                    "concept_id" => 201826,
                    "concept_name" => "Type 2 diabetes mellitus",
                    "domain_id" => "Condition",
                    "vocabulary_id" => "SNOMED",
                    "concept_class_id" => "Clinical Finding",
                    "standard_concept" => "S",
                    "concept_code" => "44054006"
                  },
                  "scores" => { "final" => 0.88, "rrf" => 0.88 },
                  "provenance" => { "search_artifact_id" => "search-test" }
                }
              ]
            }
          end
        end
      }
    end
  end

  setup do
    @user = create_user!(email: "jobs@usagi.test")
    @project = create_project!(user: @user)
    @mapping = create_mapping!(project: @project)
  end

  teardown do
    EngineClients::MapperClient.default_transport = nil
    EngineClients::JobsClient.default_transport = nil
    EngineClients::SearchClient.default_transport = nil
  end

  test "start auto map mirrors API job and preserves idempotency key" do
    transport = MapperBatchTransport.new
    EngineClients::MapperClient.default_transport = transport

    assert_enqueued_with(job: PollEngineJobJob) do
      StartAutoMapJob.perform_now(@project.id)
    end

    engine_job = @project.engine_jobs.mapper_drugs_batch.last
    assert_equal "api_mapper_batch_1", engine_job.api_job_id
    assert_equal "queued", engine_job.state
    assert_match(/\Amapper-#{@project.id}-/, engine_job.idempotency_key)
    assert_equal "/mapper/drugs/batch-job", transport.requests.first.first
    assert_equal engine_job.idempotency_key, transport.requests.first.last.fetch(:idempotency_key)
  end

  test "start auto map reuses active job with the same idempotency key" do
    transport = MapperBatchTransport.new
    EngineClients::MapperClient.default_transport = transport
    first_job = StartAutoMapJob.perform_now(@project.id)

    assert_no_difference -> { @project.engine_jobs.mapper_drugs_batch.count } do
      assert_enqueued_with(job: PollEngineJobJob, args: [first_job.id]) do
        second_job = StartAutoMapJob.perform_now(@project.id)
        assert_equal first_job, second_job
      end
    end
    assert_equal 1, transport.requests.size
  end

  test "start auto map records mapper job creation failures on the mirror job" do
    EngineClients::MapperClient.default_transport = FailingMapperBatchTransport.new

    error = assert_raises(EngineClients::BaseClient::Error) do
      StartAutoMapJob.perform_now(@project.id)
    end

    engine_job = @project.engine_jobs.mapper_drugs_batch.last
    assert_equal "ENGINE_UNAVAILABLE", error.code
    assert_equal "failed", engine_job.state
    assert_equal "ENGINE_UNAVAILABLE", engine_job.error.fetch("code")
    assert_equal "req_mapper_fail", engine_job.error.fetch("request_id")
    assert_nil engine_job.api_job_id
  end

  test "start auto map retries a local failed mirror with the same idempotency key" do
    EngineClients::MapperClient.default_transport = FailingMapperBatchTransport.new
    failed_job = assert_raises(EngineClients::BaseClient::Error) do
      StartAutoMapJob.perform_now(@project.id)
    end
    assert_equal "ENGINE_UNAVAILABLE", failed_job.code
    failed_mirror = @project.engine_jobs.mapper_drugs_batch.last

    transport = MapperBatchTransport.new("job_id" => "api_mapper_batch_retry")
    EngineClients::MapperClient.default_transport = transport

    assert_no_difference -> { @project.engine_jobs.mapper_drugs_batch.count } do
      retried = StartAutoMapJob.perform_now(@project.id)
      assert_equal failed_mirror, retried
    end

    assert_equal "api_mapper_batch_retry", failed_mirror.reload.api_job_id
    assert_equal "queued", failed_mirror.state
    assert_equal({}, failed_mirror.error)
    assert_equal 1, transport.requests.size
  end

  test "start auto map retries a partial mirror instead of treating it as reusable success" do
    partial_transport = MapperBatchTransport.new("job_id" => "api_mapper_batch_partial", "state" => "succeeded_with_errors")
    EngineClients::MapperClient.default_transport = partial_transport
    partial_mirror = StartAutoMapJob.perform_now(@project.id)

    retry_transport = MapperBatchTransport.new("job_id" => "api_mapper_batch_after_partial", "state" => "queued")
    EngineClients::MapperClient.default_transport = retry_transport

    assert_no_difference -> { @project.engine_jobs.mapper_drugs_batch.count } do
      retried = StartAutoMapJob.perform_now(@project.id)
      assert_equal partial_mirror, retried
    end

    assert_equal "api_mapper_batch_after_partial", partial_mirror.reload.api_job_id
    assert_equal "queued", partial_mirror.state
    assert_equal 1, retry_transport.requests.size
  end

  test "polling requeues active jobs and persists succeeded jobs" do
    running = @project.engine_jobs.create!(kind: "mapper_drugs_batch", state: "queued", api_job_id: "api_running")
    EngineClients::JobsClient.default_transport = JobsTransport.new(status: {
      "state" => "running",
      "processed" => 1,
      "total" => 3,
      "failed" => 0
    })

    assert_enqueued_with(job: PollEngineJobJob, args: [running.id]) do
      PollEngineJobJob.perform_now(running.id)
    end
    assert_equal "running", running.reload.state
    assert_equal 1, running.processed

    succeeded = @project.engine_jobs.create!(kind: "mapper_drugs_batch", state: "running", api_job_id: "api_succeeded")
    EngineClients::JobsClient.default_transport = JobsTransport.new(status: {
      "state" => "succeeded",
      "processed" => 3,
      "total" => 3,
      "failed" => 0
    })

    assert_enqueued_with(job: PersistMapperResultsJob, args: [succeeded.id]) do
      PollEngineJobJob.perform_now(succeeded.id)
    end
    assert_equal "succeeded", succeeded.reload.state
  end

  test "persist mapper results fetches JSONL items when the engine returns an artifact envelope" do
    engine_job = @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "succeeded",
      api_job_id: "api_jsonl_results",
      candidate_set_id: "candset_jsonl"
    )
    EngineClients::JobsClient.default_transport = JsonlResultsTransport.new(
      envelope: {
        "job_id" => "api_jsonl_results",
        "state" => "succeeded",
        "artifact" => { "content_type" => "application/jsonl", "path" => "data/jobs/results/api_jsonl_results/results.jsonl" }
      },
      jsonl: [
        {
          "id" => @mapping.source_term_id,
          "candidates" => [
            {
              "rank" => 1,
              "method" => "thirawat_tachiom_bimaxsim_tiebreak",
              "concept" => {
                "concept_id" => 40162522,
                "concept_name" => "Tramadol Hydrochloride 50 MG Oral Capsule",
                "domain_id" => "Drug",
                "vocabulary_id" => "RxNorm",
                "concept_class_id" => "Clinical Drug",
                "standard_concept" => "S",
                "concept_code" => "859751"
              },
              "scores" => { "final" => 1.0, "bimaxsim" => 1.0 },
              "features" => { "dose_form_match" => true }
            }
          ]
        }
      ]
    )

    assert_difference -> { @mapping.mapping_candidates.count }, 1 do
      PersistMapperResultsJob.perform_now(engine_job.id)
    end

    candidate = @mapping.mapping_candidates.last
    assert_equal 40162522, candidate.concept_id
    assert_equal 1.0, candidate.final_score
  end

  test "persist mapper results records JSONL stream failures on the mirror job" do
    engine_job = @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "succeeded",
      api_job_id: "api_failed_jsonl_results",
      candidate_set_id: "candset_failed_jsonl"
    )
    EngineClients::JobsClient.default_transport = FailingJsonlResultsTransport.new

    error = assert_raises(EngineClients::BaseClient::Error) do
      PersistMapperResultsJob.perform_now(engine_job.id)
    end

    engine_job.reload
    assert_equal "RESULT_STREAM_UNAVAILABLE", error.code
    assert_equal "failed", engine_job.state
    assert_equal "RESULT_STREAM_UNAVAILABLE", engine_job.error.fetch("code")
    assert_equal "req_result_stream", engine_job.error.fetch("request_id")
    assert_not_nil engine_job.finished_at
  end

  test "persist mapper results stores candidates against mappings" do
    engine_job = @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "succeeded",
      api_job_id: "api_results",
      candidate_set_id: "candset_test"
    )
    EngineClients::JobsClient.default_transport = JobsTransport.new(
      status: { "state" => "succeeded" },
      results: {
        "items" => [
          {
            "id" => @mapping.source_term_id,
            "state" => "succeeded",
            "candidates" => [
              {
                "rank" => 1,
                "method" => "thirawat_tachiom_bimaxsim_tiebreak",
                "concept" => {
                  "concept_id" => 40162522,
                  "concept_name" => "tramadol hydrochloride 50 MG Oral Capsule",
                  "domain_id" => "Drug",
                  "vocabulary_id" => "RxNorm",
                  "concept_class_id" => "Clinical Drug",
                  "standard_concept" => "S",
                  "concept_code" => "859751"
                },
                "scores" => { "final" => 0.97, "bimaxsim" => 0.96 },
                "features" => { "ingredient_match" => true },
                "provenance" => { "model_artifact_id" => "sidataplus/THIRAWAT-SapBERT" }
              }
            ]
          }
        ]
      }
    )

    assert_difference -> { @mapping.mapping_candidates.count }, 1 do
      PersistMapperResultsJob.perform_now(engine_job.id)
    end

    candidate = @mapping.mapping_candidates.last
    assert_equal engine_job, candidate.engine_job
    assert_equal "candset_test", candidate.candidate_set_id
    assert_equal 0.97, candidate.final_score
    assert_equal true, candidate.features.fetch("ingredient_match")
    assert_equal 1, @mapping.reload.candidate_count
  end

  test "persist mapper results copies failed JSONL rows to engine job error items" do
    engine_job = @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "succeeded_with_errors",
      api_job_id: "api_partial_jsonl_results",
      candidate_set_id: "candset_partial_jsonl",
      failed: 1
    )
    EngineClients::JobsClient.default_transport = JsonlResultsTransport.new(
      envelope: {
        "job_id" => "api_partial_jsonl_results",
        "state" => "succeeded_with_errors",
        "artifact" => { "content_type" => "application/jsonl", "path" => "data/jobs/results/api_partial_jsonl_results/results.jsonl" }
      },
      jsonl: [
        {
          "id" => @mapping.source_term_id,
          "source_code" => @mapping.source_code,
          "q" => "tramadol hydrochloride 50 mg capsule",
          "error" => {
            "code" => "EMBEDDING_FAILED",
            "message" => "No precomputed embedding",
            "request_id" => "req_partial_jsonl"
          }
        }
      ]
    )

    PersistMapperResultsJob.perform_now(engine_job.id)

    failed_item = engine_job.reload.error.fetch("items").first
    assert_equal @mapping.source_code, failed_item.fetch("source_code")
    assert_equal "tramadol hydrochloride 50 mg capsule", failed_item.fetch("q")
    assert_equal "EMBEDDING_FAILED", failed_item.fetch("error").fetch("code")
    assert_equal "req_partial_jsonl", failed_item.fetch("error").fetch("request_id")
  end

  test "hybrid search job records engine failures on the mirror job" do
    condition_project = create_project!(user: @user, mapping_domain: "Condition")
    create_mapping!(project: condition_project, source_code: "DX1", source_name: "diabetes mellitus")
    EngineClients::SearchClient.default_transport = FailingSearchTransport.new

    error = assert_raises(EngineClients::BaseClient::Error) do
      RunHybridSearchJob.perform_now(condition_project.id)
    end

    engine_job = condition_project.engine_jobs.hybrid_search_batch.last
    assert_equal "INDEX_NOT_READY", error.code
    assert_equal "failed", engine_job.state
    assert_equal 0, engine_job.processed
    assert_equal 1, engine_job.failed
    assert_equal "INDEX_NOT_READY", engine_job.error.fetch("code")
    assert_equal "req_fail", engine_job.error.fetch("request_id")
  end

  test "hybrid search job uses search batch chunks and records partial failures" do
    condition_project = create_project!(user: @user, mapping_domain: "Condition")
    success_one = create_mapping!(project: condition_project, source_code: "DX_OK1", source_name: "diabetes mellitus")
    failed = create_mapping!(project: condition_project, source_code: "DX_FAIL", source_name: "bad source")
    success_two = create_mapping!(project: condition_project, source_code: "DX_OK2", source_name: "type 2 diabetes")
    reviewed = create_mapping!(project: condition_project, source_code: "DX_REVIEWED", source_name: "reviewed")
    reviewed.update!(status: "approved", target_concept_id: 201826, target_concept_name: "Type 2 diabetes mellitus")
    transport = BatchSearchTransport.new
    EngineClients::SearchClient.default_transport = transport

    with_env("HYBRID_SEARCH_BATCH_SIZE" => "2") do
      RunHybridSearchJob.perform_now(condition_project.id)
    end

    engine_job = condition_project.engine_jobs.hybrid_search_batch.last
    assert_equal "succeeded_with_errors", engine_job.state
    assert_equal 3, engine_job.processed
    assert_equal 3, engine_job.total
    assert_equal 1, engine_job.failed
    assert_equal 2, transport.requests.size
    assert_equal [success_one.source_term_id, failed.source_term_id], transport.requests.first.last.fetch(:items).map { |item| item.fetch(:id) }
    assert_equal ["DX_OK1", "DX_FAIL"], transport.requests.first.last.fetch(:items).map { |item| item.fetch(:source_code) }
    assert_equal 1, success_one.reload.mapping_candidates.count
    assert_equal 0, failed.reload.mapping_candidates.count
    assert_equal 1, success_two.reload.mapping_candidates.count
    assert_equal 0, reviewed.reload.mapping_candidates.count
    assert_equal "BAD_REQUEST", engine_job.error.fetch("items").first.fetch("error").fetch("code")
  end

  test "hybrid search job uses source domain hints for mixed projects" do
    mixed_project = create_project!(user: @user, mapping_domain: "Mixed")
    mapping = create_mapping!(project: mixed_project, source_code: "RX_MIXED", source_name: "tramadol 50 mg capsule")
    mapping.source_term.update!(source_domain_hint: "Drug")
    transport = BatchSearchTransport.new
    EngineClients::SearchClient.default_transport = transport

    RunHybridSearchJob.perform_now(mixed_project.id)

    item = transport.requests.first.last.fetch(:items).first
    assert_equal({ domain_id: ["Drug"] }, item.fetch(:filters))
    assert_equal 1, mapping.reload.mapping_candidates.count
  end
end
