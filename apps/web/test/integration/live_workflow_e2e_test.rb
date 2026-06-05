require "test_helper"

class LiveWorkflowE2ETest < ActionDispatch::IntegrationTest
  include ActiveJob::TestHelper

  setup do
    skip "set USAGI_LIVE_ENGINE=1 to run against local usagi-api" unless ENV["USAGI_LIVE_ENGINE"] == "1"

    EngineClients::CatalogClient.default_transport = nil
    EngineClients::SearchClient.default_transport = nil
    EngineClients::MapperClient.default_transport = nil
    EngineClients::JobsClient.default_transport = nil
    skip "live API stack is not ready" unless live_stack_ready?

    @user = create_user!(email: "live-workflow-#{SecureRandom.hex(4)}@usagi.test", admin: true)
    sign_in_as(@user)
  end

  teardown do
    clear_enqueued_jobs
    clear_performed_jobs
    EngineClients::CatalogClient.default_transport = nil
    EngineClients::SearchClient.default_transport = nil
    EngineClients::MapperClient.default_transport = nil
    EngineClients::JobsClient.default_transport = nil
  end

  test "full Rails workflow runs against live API and renders success and partial failure job pages" do
    drug_project = create_project_via_controller!(
      name: "Live Drug Workflow #{SecureRandom.hex(4)}",
      mapping_domain: "Drug",
      source_vocabulary: "NDC",
      target_vocabulary_ids: ["RxNorm"]
    )
    import_rows!(
      drug_project,
      <<~CSV
        source_code,source_name,source_frequency
        SRC_AUGMENTIN_875_125,Augmentin 875/125,12
        SRC_MISSING,query with no precomputed embedding,4
      CSV
    )

    good_mapping = drug_project.mappings.joins(:source_term).find_by!(source_terms: { source_code: "SRC_AUGMENTIN_875_125" })
    post mapping_manual_search_path(good_mapping), params: { q: "Augmentin 875/125" }
    assert_redirected_to mapping_path(good_mapping)
    assert good_mapping.reload.mapping_candidates.exists?(concept_id: 123456)

    perform_enqueued_jobs(only: StartAutoMapJob) do
      post project_auto_map_path(drug_project)
    end
    assert_redirected_to project_engine_jobs_path(drug_project)

    mapper_job = drug_project.engine_jobs.mapper_drugs_batch.recent_first.first
    poll_mapper_job_until_terminal!(mapper_job)
    mapper_job.reload

    assert_equal "succeeded_with_errors", mapper_job.state
    assert_equal 2, mapper_job.processed
    assert_equal 1, mapper_job.failed
    assert drug_project.mapping_candidates.exists?(concept_id: 123456)
    failed_item = mapper_job.error.fetch("items").find { |item| item["source_code"] == "SRC_MISSING" }
    assert_equal "EMBEDDING_FAILED", failed_item.fetch("error").fetch("code")

    get project_engine_job_path(drug_project, mapper_job)
    assert_response :success
    assert_includes response.body, "Partial failures"
    assert_includes response.body, "EMBEDDING_FAILED"
    assert_includes response.body, "Retry"

    mixed_project = create_project_via_controller!(
      name: "Live Mixed Workflow #{SecureRandom.hex(4)}",
      mapping_domain: "Mixed",
      source_vocabulary: "source",
      target_vocabulary_ids: ["RxNorm"]
    )
    import_rows!(
      mixed_project,
      <<~CSV
        source_code,source_name,source_frequency,source_domain_hint
        MIX_AUGMENTIN,Augmentin 875/125,8,Drug
      CSV
    )

    perform_enqueued_jobs(only: RunHybridSearchJob) do
      post project_auto_map_path(mixed_project)
    end
    assert_redirected_to project_engine_jobs_path(mixed_project)

    hybrid_job = mixed_project.engine_jobs.hybrid_search_batch.recent_first.first
    mixed_mapping = mixed_project.mappings.joins(:source_term).find_by!(source_terms: { source_code: "MIX_AUGMENTIN" })
    assert_equal "succeeded", hybrid_job.state
    assert_equal 1, hybrid_job.processed
    assert_equal 0, hybrid_job.failed
    assert mixed_mapping.mapping_candidates.exists?(concept_id: 123456)

    get project_engine_job_path(mixed_project, hybrid_job)
    assert_response :success
    assert_includes response.body, "Succeeded"
    assert_includes response.body, "Review mappings"

    get project_mappings_path(mixed_project)
    assert_response :success
    assert_includes response.body, "ready with candidates"
    assert_includes response.body, "amoxicillin 875 MG / clavulanate 125 MG Oral Tablet"
  end

  private
    def live_stack_ready?
      EngineClients::CatalogClient.new.status.fetch("status") == "ready" &&
        EngineClients::SearchClient.new.status.fetch("status") == "ready" &&
        EngineClients::MapperClient.new.status.fetch("status") == "ready"
    rescue EngineClients::BaseClient::Error
      false
    end

    def create_project_via_controller!(name:, mapping_domain:, source_vocabulary:, target_vocabulary_ids:)
      post projects_path, params: {
        project: {
          name: name,
          description: "Live workflow E2E",
          mapping_domain: mapping_domain,
          source_vocabulary: source_vocabulary,
          target_vocabulary_ids: target_vocabulary_ids,
          vocabulary_version: "live-smoke"
        }
      }
      assert_response :redirect
      Project.find_by!(name: name)
    end

    def import_rows!(project, rows_text)
      post project_import_sessions_path(project), params: {
        import_session: {
          filename: "live-workflow.csv",
          rows_text: rows_text
        }
      }
      assert_response :redirect
      assert_equal "succeeded", project.import_sessions.recent_first.first.state
    end

    def poll_mapper_job_until_terminal!(engine_job)
      deadline = 30.seconds.from_now
      loop do
        perform_enqueued_jobs(only: PersistMapperResultsJob) do
          PollEngineJobJob.perform_now(engine_job.id)
        end
        engine_job.reload
        if engine_job.state.in?(%w[succeeded succeeded_with_errors])
          PersistMapperResultsJob.perform_now(engine_job.id) if engine_job.result.blank?
          return
        end
        return if engine_job.state.in?(%w[failed cancelled])
        flunk "timed out waiting for #{engine_job.api_job_id}" if Time.current > deadline

        sleep 0.5
      end
    end
end
