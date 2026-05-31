require "test_helper"

class WorkflowControllersTest < ActionDispatch::IntegrationTest
  setup do
    @user = create_user!(email: "demo@usagi.test", admin: true)
    @project = create_project!(user: @user)
    sign_in_as(@user)
  end

  test "import creates source terms and review mappings" do
    post project_import_sessions_path(@project), params: {
      import_session: {
        filename: "drugs.csv",
        rows_text: "source_code,source_name,source_frequency\nSRC_TRAMADOL,tramadol hcl 50mg cap,12\nSRC_METFORMIN,metformin hcl 500 mg tab,7\n"
      }
    }

    assert_redirected_to project_import_session_path(@project, ImportSession.last)
    assert_equal 2, @project.source_terms.count
    assert_equal 2, @project.mappings.count
    assert_equal "UNCHECKED", @project.mappings.first.mapping_status
  end

  test "import persists valid rows and reports invalid row failures" do
    post project_import_sessions_path(@project), params: {
      import_session: {
        filename: "mixed.csv",
        rows_text: "source_code,source_name,source_frequency\nSRC_DUP,metformin hcl 500 mg tab,12\nSRC_DUP,duplicate metformin,3\nSRC_BLANK,,1\nSRC_OK,tramadol hcl 50mg cap,7\n"
      }
    }

    import = ImportSession.last
    assert_redirected_to project_import_session_path(@project, import)
    assert_equal "succeeded_with_errors", import.state
    assert_equal({ "rows_seen" => 4, "rows_imported" => 2, "rows_failed" => 2 }, import.summary.slice("rows_seen", "rows_imported", "rows_failed"))
    assert_equal 2, import.error.fetch("row_errors").size
    assert_equal ["SRC_DUP", "SRC_OK"], @project.source_terms.order(:source_code).pluck(:source_code)
  end

  test "manual search persists candidates for a mapping" do
    mapping = create_mapping!(project: @project)

    post mapping_manual_search_path(mapping), params: { q: "tramadol hcl 50mg cap" }

    assert_redirected_to mapping_path(mapping)
    assert_equal 1, @project.engine_jobs.mapper_drugs_batch.count
    assert mapping.mapping_candidates.exists?
    assert_equal "manual_search", @project.audit_events.last.action
  end

  test "applying a candidate sets target but leaves status unchecked" do
    mapping = create_mapping!(project: @project)
    candidate = mapping.mapping_candidates.create!(
      project: @project,
      source_term: mapping.source_term,
      concept_id: 40162522,
      concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      domain_id: "Drug",
      vocabulary_id: "RxNorm",
      rank: 1,
      final_score: 0.91,
      method: "thirawat_tachiom_bimaxsim_tiebreak"
    )

    patch mapping_candidate_path(candidate)

    assert_redirected_to mapping_path(mapping)
    assert candidate.reload.selected?
    assert_equal 40162522, mapping.reload.target_concept_id
    assert_equal "UNCHECKED", mapping.mapping_status
  end

  test "manual target update does not auto approve mapping" do
    mapping = create_mapping!(project: @project)

    patch mapping_path(mapping), params: {
      mapping: {
        concept_id: 40162522,
        concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
        vocabulary_id: "RxNorm",
        domain_id: "Drug",
        equivalence: "Equivalent"
      }
    }

    assert_redirected_to mapping_path(mapping)
    assert_equal 40162522, mapping.reload.target_concept_id
    assert_equal "UNCHECKED", mapping.mapping_status
    assert_equal "Equivalent", mapping.equivalence
  end

  test "auto map queues a project level engine workflow" do
    create_mapping!(project: @project)

    assert_enqueued_jobs 1 do
      post project_auto_map_path(@project)
    end

    assert_redirected_to project_engine_jobs_path(@project)
    assert_equal "auto_map_requested", @project.audit_events.last.action
  end

  test "CSV export records export and audit event" do
    create_mapping!(project: @project)

    get project_export_path(@project, "inline", format: :csv)

    assert_response :success
    assert_equal "text/csv", response.media_type
    assert_equal 1, @project.exports.count
    assert_equal "export_generated", @project.audit_events.last.action
  end

  test "admin engine status renders with stubbed service statuses" do
    get admin_engine_status_path

    assert_response :success
    assert_includes response.body, "Engine status"
    assert_includes response.body, "Catalog"
  end

  test "failed engine job can be retried from job detail" do
    engine_job = @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "failed",
      error: { code: "JOB_FAILED", message: "Mapper failed" },
      input: { source_engine_job_id: "old" }
    )

    get project_engine_job_path(@project, engine_job)

    assert_response :success
    assert_includes response.body, "Retry"
    assert_includes response.body, "Mapper failed"

    assert_enqueued_with(job: StartAutoMapJob, args: [@project.id]) do
      post retry_project_engine_job_path(@project, engine_job)
    end
    assert_redirected_to project_engine_jobs_path(@project)
    assert_equal "engine_job_retry_requested", @project.audit_events.last.action
  end
end
