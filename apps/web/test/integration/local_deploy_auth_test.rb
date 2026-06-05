require "test_helper"

class LocalDeployAuthIntegrationTest < ActionDispatch::IntegrationTest
  test "local deployment auto signs in without a password" do
    with_env("USAGI_LOCAL_DEPLOY" => "1") do
      get projects_path

      assert_response :success
      assert_match "Local Reviewer", response.body
      assert_no_match "Sign in", response.body
    end
  end

  test "local deployment redirects sign in to projects" do
    with_env("USAGI_LOCAL_DEPLOY" => "1") do
      get new_session_path

      assert_redirected_to projects_path
    end
  end
end
