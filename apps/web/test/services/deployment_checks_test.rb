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

  test "production requires Rails secrets database and all engine URLs" do
    errors = DeploymentChecks.production_errors(
      {
        "USAGI_API_SHARED_SECRET" => "secret",
        "SEARCH_API_URL" => "http://search-api:8789"
      }
    )

    assert_includes errors, "SECRET_KEY_BASE is required in production"
    assert_includes errors, "DATABASE_URL is required in production"
    assert_includes errors, "CATALOG_API_URL is required in production"
    assert_includes errors, "MAPPER_API_URL is required in production"
    assert_includes errors, "JOBS_API_URL is required in production"
  end

  test "production engine URLs must target private service hosts" do
    errors = DeploymentChecks.production_errors(
      {
        "USAGI_API_SHARED_SECRET" => "secret",
        "SECRET_KEY_BASE" => "rails-secret",
        "DATABASE_URL" => "postgres://db/usagi",
        "CATALOG_API_URL" => "https://catalog.example.com",
        "SEARCH_API_URL" => "http://search-api:8789",
        "MAPPER_API_URL" => "http://10.0.0.20:8790",
        "JOBS_API_URL" => "https://jobs.internal"
      }
    )

    assert_includes errors, "CATALOG_API_URL must point at a private engine host in production"
    assert_not_includes errors, "SEARCH_API_URL must point at a private engine host in production"
    assert_not_includes errors, "MAPPER_API_URL must point at a private engine host in production"
    assert_not_includes errors, "JOBS_API_URL must point at a private engine host in production"
  end
end
