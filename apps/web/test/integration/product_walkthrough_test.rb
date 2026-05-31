require "test_helper"

class ProductWalkthroughTest < ActionDispatch::IntegrationTest
  setup do
    @user = create_user!(email: "walkthrough@usagi.test", admin: true)
    @project = create_project!(user: @user)
    sign_in_as(@user)
  end

  test "reviewer can import, search, apply, approve, comment, export, and inspect audit trail" do
    post project_import_sessions_path(@project), params: {
      import_session: {
        filename: "walkthrough.csv",
        rows_text: "source_code,source_name,source_frequency\nSRC_TRAMADOL,tramadol hcl 50mg cap,12\n"
      }
    }

    import = @project.import_sessions.last
    mapping = @project.mappings.includes(:source_term).last

    assert_redirected_to project_import_session_path(@project, import)
    assert_equal "SRC_TRAMADOL", mapping.source_code
    assert_equal "UNCHECKED", mapping.mapping_status

    post mapping_manual_search_path(mapping), params: { q: mapping.source_name }

    assert_redirected_to mapping_path(mapping)
    assert_equal 1, @project.engine_jobs.mapper_drugs_batch.count
    assert mapping.mapping_candidates.exists?

    candidate = mapping.mapping_candidates.ranked.first
    patch mapping_candidate_path(candidate)

    assert_redirected_to mapping_path(mapping)
    assert candidate.reload.selected?
    assert_equal candidate.concept_id, mapping.reload.target_concept_id
    assert_equal "UNCHECKED", mapping.mapping_status

    patch status_mapping_path(mapping, status: "approved")

    assert_redirected_to project_mappings_path(@project)
    assert_equal "APPROVED", mapping.reload.mapping_status
    assert_equal @user, mapping.reviewed_by

    post mapping_comments_path(mapping), params: { comment: { content: "Reviewed against source context." } }

    assert_redirected_to mapping_path(mapping)
    assert_equal 1, mapping.comments.count

    get project_export_path(@project, "inline", format: :csv)

    assert_response :success
    assert_equal "text/csv", response.media_type
    assert_includes response.body, "SRC_TRAMADOL"
    assert_includes response.body, candidate.concept_name

    actions = @project.audit_events.order(:created_at).pluck(:action)
    assert_includes actions, "import_created"
    assert_includes actions, "manual_search"
    assert_includes actions, "candidate_applied"
    assert_includes actions, "status_changed"
    assert_includes actions, "commented"
    assert_includes actions, "export_generated"
  end
end
