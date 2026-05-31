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

  test "import preview detects columns without creating source terms" do
    post preview_project_import_sessions_path(@project), params: {
      import_session: {
        filename: "detected.csv",
        rows_text: "Code,Description,Count,Domain\nSRC_TRAMADOL,tramadol hcl 50mg cap,12,Drug\nSRC_METFORMIN,metformin hcl 500 mg tab,7,Drug\n"
      }
    }

    import = @project.import_sessions.last

    assert_response :success
    assert_equal "previewed", import.state
    assert_equal "Code", import.column_mapping.fetch("source_code")
    assert_equal "Description", import.column_mapping.fetch("source_name")
    assert_equal %w[Code Description Count Domain], import.detected_columns
    assert_equal({ "rows_seen" => 2, "preview_rows" => 2 }, import.summary.slice("rows_seen", "preview_rows"))
    assert_equal 0, @project.source_terms.count
    assert_includes response.body, "Import preview"
    assert_includes response.body, "Confirm import"
    assert_includes response.body, "SRC_TRAMADOL"
  end

  test "confirm imports previewed rows using detected column mapping" do
    post preview_project_import_sessions_path(@project), params: {
      import_session: {
        filename: "detected.csv",
        rows_text: "Code,Description,Count,Domain\nSRC_TRAMADOL,tramadol hcl 50mg cap,12,Drug\n"
      }
    }
    import = @project.import_sessions.last

    post confirm_project_import_session_path(@project, import), params: {
      import_session: {
        column_mapping: import.column_mapping
      }
    }

    assert_redirected_to project_import_session_path(@project, import)
    follow_redirect!
    assert_response :success
    assert_includes response.body, "Suggest candidates"
    assert_equal "succeeded", import.reload.state
    assert_equal ["SRC_TRAMADOL"], @project.source_terms.pluck(:source_code)
    assert_equal "tramadol hcl 50mg cap", @project.source_terms.last.source_name
    assert_equal "Drug", @project.source_terms.last.source_domain_hint
    assert_equal 1, @project.mappings.count
  end

  test "project overview centers the mapping review queue" do
    create_mapping!(project: @project, source_code: "SRC_TRAMADOL")
    @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "succeeded_with_errors",
      processed: 9,
      total: 10,
      failed: 1,
      stage: "persist_candidates",
      input: { idempotency_key: "overview-suggestions" }
    )

    get project_path(@project)

    assert_response :success
    assert_includes response.body, "Review queue"
    assert_includes response.body, "Review unchecked mappings"
    assert_includes response.body, "Suggest candidates"
    assert_includes response.body, "Latest suggestions"
    assert_includes response.body, "9 of 10"
    assert_includes response.body, "1 failed"
    assert_includes response.body, "Export reviewed mappings"
  end

  test "project overview does not show fake suggestion progress when counts are absent" do
    create_mapping!(project: @project)
    @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "succeeded",
      processed: 0,
      total: 0,
      input: { idempotency_key: "overview-empty-progress" }
    )

    get project_path(@project)

    assert_response :success
    assert_includes response.body, "No row counts reported"
    refute_includes response.body, "0 of 0"
  end

  test "import result hands reviewers to suggestions and review" do
    post project_import_sessions_path(@project), params: {
      import_session: {
        filename: "drugs.csv",
        rows_text: "source_code,source_name,source_frequency\nSRC_TRAMADOL,tramadol hcl 50mg cap,12\n"
      }
    }
    import = @project.import_sessions.last

    get project_import_session_path(@project, import)

    assert_response :success
    assert_includes response.body, "Ready for review"
    assert_includes response.body, "1 source term became a mapping"
    assert_includes response.body, "Suggest candidates"
    assert_includes response.body, "Review imported terms"
    refute_includes response.body, "Auto-map"
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

  test "engine jobs pages route suggestion work back to review" do
    create_mapping!(project: @project)
    job = @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "succeeded",
      processed: 0,
      total: 0,
      input: { idempotency_key: "job-review-flow" }
    )

    get project_engine_jobs_path(@project)

    assert_response :success
    assert_includes response.body, "Suggestion jobs feed the Review workspace"
    assert_includes response.body, "Review mappings"
    assert_includes response.body, "Not reported"
    refute_includes response.body, "0/0"

    get project_engine_job_path(@project, job)

    assert_response :success
    assert_includes response.body, "Candidates appear on mapping rows"
    assert_includes response.body, "Review mappings"
    assert_includes response.body, "Not reported"
    refute_includes response.body, "0/0"
  end

  test "exports page frames downloads as reviewed mapping output" do
    create_mapping!(project: @project)
    export = @project.exports.create!(
      requested_by: @user,
      format: "review_csv",
      state: "succeeded",
      row_count: 1,
      file_key: "review.csv",
      finished_at: Time.current
    )

    get project_exports_path(@project)

    assert_response :success
    assert_includes response.body, "Export reviewed mappings"
    assert_includes response.body, "Review before exporting"
    assert_includes response.body, "Review mappings"

    get project_export_path(@project, export)

    assert_response :success
    assert_includes response.body, "Export mappings"
    assert_includes response.body, "Review mappings"
  end

  test "manual search turbo response keeps fast next action" do
    mapping = create_mapping!(project: @project, source_code: "SRC_A")
    create_mapping!(project: @project, source_code: "SRC_B", source_name: "metformin hcl 500 mg tab")

    post mapping_manual_search_path(mapping),
      params: { q: "tramadol hcl 50mg cap" },
      headers: { "Accept" => "text/vnd.turbo-stream.html" }

    assert_response :success
    assert_includes response.body, "Use and next"
    assert_includes response.body, "saved candidates"
  end

  test "mapping detail renders audit history after manual search" do
    mapping = create_mapping!(project: @project)

    post mapping_manual_search_path(mapping), params: { q: "tramadol hcl 50mg cap" }
    get mapping_path(mapping)

    assert_response :success
    assert_includes response.body, "Manual search"
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

  test "mapping review shows real suggest action and latest engine job status" do
    create_mapping!(project: @project)
    @project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "running",
      processed: 2,
      total: 4,
      failed: 1,
      stage: "bimaxsim_rerank",
      input: { idempotency_key: "mapper-demo" }
    )

    get project_mappings_path(@project)

    assert_response :success
    assert_includes response.body, "Suggest candidates"
    assert_includes response.body, "Drug mapper"
    assert_includes response.body, "Running"
    assert_includes response.body, "2 of 4"
    assert_includes response.body, "1 failed"
    assert_includes response.body, "bimaxsim_rerank"
    refute_includes response.body, "Wiring up the job is coming soon"
  end

  test "candidate detail explains evidence and offers fast next actions" do
    mapping = create_mapping!(project: @project, source_code: "SRC_A")
    create_mapping!(project: @project, source_code: "SRC_B", source_name: "metformin hcl 500 mg tab")
    mapping.mapping_candidates.create!(
      project: @project,
      source_term: mapping.source_term,
      rank: 1,
      concept_id: 40162522,
      concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      domain_id: "Drug",
      vocabulary_id: "RxNorm",
      concept_class_id: "Clinical Drug",
      standard_concept: "S",
      concept_code: "859751",
      method: "thirawat_tachiom_bimaxsim_tiebreak",
      final_score: 0.93,
      bimaxsim_score: 0.88,
      tachiom_maxsim_score: 0.84,
      features: { "ingredient_match" => true },
      provenance: { "candidate_set_id" => "candset_1", "model" => "THIRAWAT-SapBERT" },
      warnings: ["dose form differs"]
    )

    get mapping_path(mapping)

    assert_response :success
    assert_includes response.body, "Review cockpit"
    assert_includes response.body, "Why this candidate"
    assert_includes response.body, "Final 0.93"
    assert_includes response.body, "BiMaxSim 0.88"
    assert_includes response.body, "Tachiom 0.84"
    assert_includes response.body, "ingredient_match"
    assert_includes response.body, "THIRAWAT-SapBERT"
    assert_includes response.body, "dose form differs"
    assert_includes response.body, "Use and next"
    assert_includes response.body, "Approve and next"
  end

  test "candidate can be applied and advanced to the next mapping" do
    mapping = create_mapping!(project: @project, source_code: "SRC_A")
    next_mapping = create_mapping!(project: @project, source_code: "SRC_B", source_name: "metformin hcl 500 mg tab")
    candidate = mapping.mapping_candidates.create!(
      project: @project,
      source_term: mapping.source_term,
      rank: 1,
      concept_id: 40162522,
      concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      method: "hybrid_rrf"
    )

    patch mapping_candidate_path(candidate, next: "1")

    assert_redirected_to mapping_path(next_mapping)
    assert candidate.reload.selected?
    assert_equal 40162522, mapping.reload.target_concept_id
    assert_equal "UNCHECKED", mapping.mapping_status
  end

  test "mapping can be approved and advanced to the next mapping" do
    mapping = create_mapping!(project: @project, source_code: "SRC_A")
    next_mapping = create_mapping!(project: @project, source_code: "SRC_B", source_name: "metformin hcl 500 mg tab")

    patch mapping_path(mapping, next: "1"), params: { mapping: { status: "approved" } }

    assert_redirected_to mapping_path(next_mapping)
    assert_equal "APPROVED", mapping.reload.mapping_status
    assert_equal @user, mapping.reviewed_by
  end

  test "CSV export records export and audit event" do
    create_mapping!(project: @project)

    get project_export_path(@project, "inline", format: :csv)

    assert_response :success
    assert_equal "text/csv", response.media_type
    assert_equal 1, @project.exports.count
    assert_equal "export_generated", @project.audit_events.last.action
  end

  test "candidate JSONL export downloads newline-delimited candidate provenance" do
    mapping = create_mapping!(project: @project)
    mapping.mapping_candidates.create!(
      project: @project,
      source_term: mapping.source_term,
      rank: 1,
      concept_id: 40162522,
      concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      method: "hybrid_rrf",
      final_score: 0.91,
      provenance: { "search_artifact_id" => "search-v1" }
    )
    export = @project.exports.create!(
      requested_by: @user,
      format: "candidate_jsonl",
      state: "succeeded",
      row_count: 1,
      file_key: "candidate-export.jsonl"
    )

    get project_export_path(@project, export, format: :jsonl)

    assert_response :success
    assert_equal "application/x-ndjson", response.media_type
    line = JSON.parse(response.body.lines.first)
    assert_equal "SRC", line.fetch("source_code")
    assert_equal "search-v1", line.fetch("provenance").fetch("search_artifact_id")
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

  test "partial engine job shows failed row details and retry action" do
    engine_job = @project.engine_jobs.create!(
      kind: "hybrid_search_batch",
      state: "succeeded_with_errors",
      processed: 3,
      total: 3,
      failed: 1,
      stage: "persist_results",
      error: {
        items: [
          {
            source_code: "DX_FAIL",
            error: {
              code: "BAD_REQUEST",
              message: "Cannot map source term",
              request_id: "req_partial"
            }
          }
        ]
      },
      input: { source_engine_job_id: "old-hybrid" }
    )

    get project_engine_job_path(@project, engine_job)

    assert_response :success
    assert_includes response.body, "Succeeded with errors"
    assert_includes response.body, "persist_results"
    assert_includes response.body, "Partial failures"
    assert_includes response.body, "DX_FAIL"
    assert_includes response.body, "BAD_REQUEST"
    assert_includes response.body, "Cannot map source term"
    assert_includes response.body, "req_partial"
    assert_includes response.body, "Retry"
  end
end
