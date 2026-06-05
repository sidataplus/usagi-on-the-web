require "test_helper"

class LocalDeployAuthTest < ActiveSupport::TestCase
  test "ensure_user creates a single admin reviewer without a usable password workflow" do
    with_env("USAGI_LOCAL_DEPLOY" => "1", "USAGI_LOCAL_DEPLOY_USER_EMAIL" => "local-deploy@example.test") do
      user = LocalDeployAuth.ensure_user!

      assert_equal "local-deploy@example.test", user.email
      assert user.admin?
      assert_predicate user.password_digest, :present?

      assert_equal user.id, LocalDeployAuth.ensure_user!.id
    end
  end

  test "sign_in stores the local reviewer in the session" do
    with_env("USAGI_LOCAL_DEPLOY" => "1") do
      session = {}
      user = LocalDeployAuth.sign_in!(session)

      assert_equal user.id, session[:user_id]
    end
  end

  test "local deployment seeds source terms covered by demo mapper artifacts" do
    with_env("USAGI_LOCAL_DEPLOY" => "1", "USAGI_LOCAL_DEPLOY_USER_EMAIL" => "local-seed@example.test") do
      load Rails.root.join("db/seeds.rb")

      project = Project.find_by!(name: "Demo: Augmentin Drug Review")
      assert_equal "Drug", project.mapping_domain
      assert_equal ["RxNorm"], project.target_vocabulary_ids
      assert_equal(
        [
          ["SRC_AUGMENTIN_875_125", "Augmentin 875/125"]
        ],
        project.source_terms.order(:source_row_number).pluck(:source_code, :source_name)
      )
      assert_equal 1, project.mappings.unchecked.count
    end
  end
end
