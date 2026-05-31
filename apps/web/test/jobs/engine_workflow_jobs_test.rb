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

  class FailingSearchTransport
    def search_concepts(**)
      raise EngineClients::BaseClient::Error.new(code: "INDEX_NOT_READY", message: "Search index is not built", request_id: "req_fail")
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
    assert_equal "INDEX_NOT_READY", engine_job.error.fetch("code")
    assert_equal "req_fail", engine_job.error.fetch("request_id")
  end
end
