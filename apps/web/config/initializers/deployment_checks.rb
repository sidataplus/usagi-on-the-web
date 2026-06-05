if Rails.env.production? && ENV["SECRET_KEY_BASE_DUMMY"].blank?
  require Rails.root.join("app/services/deployment_checks").to_s

  deployment_errors = DeploymentChecks.production_errors
  raise deployment_errors.join("; ") if deployment_errors.any?
end
