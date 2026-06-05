require "uri"
require "ipaddr"

class DeploymentChecks
  ENGINE_URL_KEYS = %w[CATALOG_API_URL SEARCH_API_URL MAPPER_API_URL JOBS_API_URL].freeze
  REQUIRED_SECRET_KEYS = %w[USAGI_API_SHARED_SECRET SECRET_KEY_BASE DATABASE_URL].freeze

  def self.production_errors(env = ENV)
    errors = []
    REQUIRED_SECRET_KEYS.each do |key|
      errors << "#{key} is required in production" if env[key].to_s.blank?
    end

    ENGINE_URL_KEYS.each do |key|
      value = env[key].to_s
      if value.blank?
        errors << "#{key} is required in production"
        next
      end

      errors << "#{key} must not point at localhost in production" if localhost_url?(value)
      errors << "#{key} must point at a private engine host in production" unless private_engine_url?(value)
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

  def self.private_engine_url?(value)
    host = URI.parse(value).host.to_s
    return false if host.blank?
    return true if host.exclude?(".")
    return true if host.end_with?(".internal")
    return true if tailscale_host?(host)

    private_ip?(host)
  rescue URI::InvalidURIError
    false
  end
  private_class_method :private_engine_url?

  def self.tailscale_host?(host)
    host.end_with?(".ts.net")
  end
  private_class_method :tailscale_host?

  def self.private_ip?(host)
    ip = IPAddr.new(host)
    PRIVATE_IP_RANGES.any? { |range| range.include?(ip) }
  rescue IPAddr::InvalidAddressError
    false
  end
  private_class_method :private_ip?

  PRIVATE_IP_RANGES = [
    IPAddr.new("10.0.0.0/8"),
    IPAddr.new("100.64.0.0/10"), # RFC 6598 shared space; Tailscale CGNAT range
    IPAddr.new("172.16.0.0/12"),
    IPAddr.new("192.168.0.0/16"),
    IPAddr.new("fc00::/7"),
    IPAddr.new("fe80::/10")
  ].freeze
  private_constant :PRIVATE_IP_RANGES
end
