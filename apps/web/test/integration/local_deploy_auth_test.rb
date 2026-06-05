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

  test "local deployment seed includes the Augmentin review candidates" do
    with_env("USAGI_LOCAL_DEPLOY" => "1") do
      load Rails.root.join("db/seeds.rb")
    end

    project = Project.find_by!(name: "Demo: Augmentin Drug Review")
    mapping = project.mappings.joins(:source_term)
                     .find_by!(source_terms: { source_code: "SRC_AUGMENTIN_875_125" })
    candidates = mapping.mapping_candidates.ranked

    assert_equal "Augmentin 875/125", mapping.source_name
    assert_equal 3, candidates.count
    assert_equal 123456, candidates.first.concept_id
    assert_equal "amoxicillin 875 MG / clavulanate 125 MG Oral Tablet", candidates.first.concept_name
    assert_equal "thirawat_tachiom_bimaxsim_tiebreak", candidates.first.method
    assert_equal 3, mapping.reload.candidate_count
  end
end
