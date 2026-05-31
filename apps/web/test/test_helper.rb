ENV["RAILS_ENV"] ||= "test"
require_relative "../config/environment"
require "rails/test_help"
require "securerandom"

module ActiveSupport
  class TestCase
    # Run tests in parallel with specified workers
    parallelize(workers: :number_of_processors)

    # Setup all fixtures in test/fixtures/*.yml for all tests in alphabetical order.
    fixtures :all

    # Add more helper methods to be used by all tests here...
    def sign_in_as(user)
      post session_path, params: { email: user.email, password: "password123" }
      assert_response :redirect
    end

    def create_user!(email: "user#{SecureRandom.hex(4)}@example.test", admin: false)
      User.create!(
        email: email,
        name: email.split("@").first.humanize,
        password: "password123",
        password_confirmation: "password123",
        admin: admin
      )
    end

    def create_project!(user:, mapping_domain: "Drug")
      Project.create!(
        name: "Project #{SecureRandom.hex(3)}",
        created_by: user,
        mapping_domain: mapping_domain,
        source_vocabulary: mapping_domain == "Drug" ? "NDC" : "ICD10CM",
        target_vocabulary_ids: mapping_domain == "Drug" ? ["RxNorm"] : ["SNOMED"]
      )
    end

    def create_mapping!(project:, source_code: "SRC", source_name: "tramadol hcl 50mg cap")
      import = project.import_sessions.create!(created_by: project.created_by, file_name: "test.csv", state: "succeeded")
      term = project.source_terms.create!(
        import_session: import,
        source_code: source_code,
        source_name: source_name,
        source_frequency: 12,
        source_row_number: 1
      )
      project.mappings.create!(source_term: term, mapping_status: "UNCHECKED")
    end

    def with_env(values)
      old_values = values.keys.index_with { |key| ENV[key] }
      values.each { |key, value| ENV[key] = value }
      yield
    ensure
      old_values.each do |key, value|
        value.nil? ? ENV.delete(key) : ENV[key] = value
      end
    end
  end
end

class ActionDispatch::IntegrationTest
  def sign_in_as(user, password: "password123")
    user.update!(password: password, password_confirmation: password) if user.password_digest.blank?
    post session_path, params: { email: user.email, password: password }
    assert_redirected_to projects_path
  end

  def create_mapping(project, code:, name:, status: "unchecked", frequency: 0, target: {})
    term = project.source_terms.create!(
      source_code: code,
      source_name: name,
      source_frequency: frequency
    )
    project.mappings.create!({ source_term: term, status: status }.merge(target))
  end
end
