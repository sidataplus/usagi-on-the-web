require "base64"
require "digest"
require "openssl"
require "securerandom"

module EngineClients
  class RequestSigner
    def initialize(secret: ENV.fetch("USAGI_API_SHARED_SECRET", "test-secret"))
      @secret = secret
    end

    def headers(method:, path_with_query:, body:, request_id:)
      timestamp = Time.now.utc.iso8601
      nonce = "nonce_#{SecureRandom.hex(16)}"
      content_sha = Digest::SHA256.hexdigest(body.to_s)
      canonical = [
        method.to_s.upcase,
        path_with_query,
        timestamp,
        nonce,
        request_id,
        content_sha
      ].join("\n")
      signature = Base64.strict_encode64(OpenSSL::HMAC.digest("SHA256", @secret, canonical))

      {
        "X-Request-Id" => request_id,
        "X-Usagi-Service" => "rails",
        "X-Usagi-Timestamp" => timestamp,
        "X-Usagi-Nonce" => nonce,
        "X-Usagi-Content-SHA256" => content_sha,
        "X-Usagi-Signature" => "v1=#{signature}"
      }
    end
  end
end
