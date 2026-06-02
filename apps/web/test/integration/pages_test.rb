require "test_helper"

class PagesTest < ActionDispatch::IntegrationTest
  setup do
    @admin = create_user!(email: "admin@usagi.test", admin: true)
    @project = create_project!(user: @admin)
    @mapping = create_mapping!(project: @project)
    sign_in_as(@admin)
  end

  test "authenticated top level pages render" do
    [projects_path, new_project_path, dashboard_path, settings_path,
     admin_root_path, admin_users_path, admin_audit_logs_path,
     project_path(@project), project_mappings_path(@project),
     project_engine_jobs_path(@project), project_import_sessions_path(@project),
     project_exports_path(@project)].each do |path|
      get path
      assert_response :success, "expected 200 for #{path}, got #{response.status}"
    end
  end

  test "anonymous users are redirected to sign in" do
    delete session_path
    get projects_path

    assert_redirected_to new_session_path
  end

  test "projects page leads with the next project action" do
    get projects_path

    assert_response :success
    assert_select ".next-action-panel", text: /Next step/
    assert_select ".quiet-section", minimum: 1
  end

  test "settings page lets the reviewer save preferences" do
    get settings_path

    assert_response :success
    assert_select ".details-panel", minimum: 1
    assert_includes response.body, "Rows per page"
    assert_includes response.body, "Save preferences"
  end

  test "mapping detail and status update render" do
    get mapping_path(@mapping)
    assert_response :success
    assert_match @mapping.source_name, response.body

    patch status_mapping_path(@mapping, status: "approved"), headers: { "Accept" => "text/vnd.turbo-stream.html" }
    assert_response :success
    assert_equal "APPROVED", @mapping.reload.mapping_status
  end

  test "comments append via turbo stream" do
    assert_difference -> { @mapping.comments.count }, 1 do
      post mapping_comments_path(@mapping, comment: { content: "Looks right" }),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }
    end
    assert_response :success
  end

  test "unknown path returns 404 without authentication loop" do
    get "/this-page-does-not-exist"

    assert_response :not_found
  end
end
