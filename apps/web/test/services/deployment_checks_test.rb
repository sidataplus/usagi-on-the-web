require "test_helper"

class DeploymentChecksTest < ActiveSupport::TestCase
  test "production requires a shared API secret and HTTP engine URLs off localhost" do
    errors = DeploymentChecks.production_errors(
      {
        "USAGI_API_SHARED_SECRET" => "",
        "CATALOG_API_URL" => "http://127.0.0.1:8788",
        "SEARCH_API_URL" => "http://search-api:8789",
        "MAPPER_API_URL" => "https://mapper.internal"
      }
    )

    assert_includes errors, "USAGI_API_SHARED_SECRET is required in production"
    assert_includes errors, "CATALOG_API_URL must not point at localhost in production"
    assert_not_includes errors, "SEARCH_API_URL must not point at localhost in production"
  end
end
