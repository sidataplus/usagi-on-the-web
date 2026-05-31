require "uri"

class DeploymentChecks
  ENGINE_URL_KEYS = %w[CATALOG_API_URL SEARCH_API_URL MAPPER_API_URL JOBS_API_URL].freeze

  def self.production_errors(env = ENV)
    errors = []
    errors << "USAGI_API_SHARED_SECRET is required in production" if env["USAGI_API_SHARED_SECRET"].to_s.blank?

    ENGINE_URL_KEYS.each do |key|
      value = env[key].to_s
      next if value.blank?

      errors << "#{key} must not point at localhost in production" if localhost_url?(value)
    end

    errors
  end

  def self.localhost_url?(value)
    uri = URI.parse(value)
    uri.host.in?(%w[localhost 127.0.0.1 0.0.0.0])
  rescue URI::InvalidURIError
    false
  end
  private_class_method :localhost_url?
end
