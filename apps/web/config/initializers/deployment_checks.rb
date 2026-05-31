if Rails.env.production?
  deployment_errors = DeploymentChecks.production_errors
  raise deployment_errors.join("; ") if deployment_errors.any?
end
