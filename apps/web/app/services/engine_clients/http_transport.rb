require "json"
require "net/http"
require "securerandom"
require "uri"

module EngineClients
  class HttpTransport
    TRANSIENT_CONNECTION_ERRORS = [
      Timeout::Error,
      Errno::ECONNREFUSED,
      Errno::ECONNRESET,
      EOFError,
      SocketError
    ].freeze

    def initialize(base_url:, signer: RequestSigner.new, timeout: ENV.fetch("ENGINE_API_TIMEOUT_SECONDS", 30).to_f,
                   open_timeout: ENV.fetch("ENGINE_API_OPEN_TIMEOUT_SECONDS", 5).to_f,
                   max_get_retries: ENV.fetch("ENGINE_API_GET_RETRIES", 1).to_i,
                   http_client: Net::HTTP,
                   logger: Rails.logger)
      @base_url = base_url
      @signer = signer
      @timeout = timeout
      @open_timeout = open_timeout
      @max_get_retries = [max_get_retries, 0].max
      @http_client = http_client
      @logger = logger
    end

    def get_json(path)
      request_json(Net::HTTP::Get, path, nil, retryable: true)
    end

    def post_json(path, payload)
      request_json(Net::HTTP::Post, path, payload, retryable: false)
    end

    # Fetch a newline-delimited JSON stream (e.g. mapper batch results) and
    # return one parsed object per line. The engine serves these only when asked
    # with Accept: application/jsonl; the default Accept returns an envelope.
    def get_jsonl(path)
      uri = URI.join(base_url, path)
      request = Net::HTTP::Get.new(uri)
      request["Accept"] = "application/jsonl"
      request["X-API-Key"] = ENV["USAGI_API_KEY"] if ENV["USAGI_API_KEY"].present?
      request["Authorization"] = "Bearer #{ENV["USAGI_API_BEARER_TOKEN"]}" if ENV["USAGI_API_BEARER_TOKEN"].present?
      signer.headers(
        method: request.method,
        path_with_query: uri.request_uri,
        body: "",
        request_id: Current.request_id || SecureRandom.uuid
      ).each { |key, value| request[key] = value }

      response = perform_request(uri, request, retryable: true)
      unless response.is_a?(Net::HTTPSuccess)
        envelope = response.body.present? ? JSON.parse(response.body) : {}
        raise BaseClient::Error.from_envelope(envelope, status: response.code.to_i)
      end

      response.body.to_s.each_line.filter_map do |line|
        stripped = line.strip
        JSON.parse(stripped) unless stripped.empty?
      end
    rescue JSON::ParserError => e
      raise BaseClient::Error.new(code: "INTERNAL_ERROR", message: "Invalid engine JSONL: #{e.message}", status: 502)
    rescue *TRANSIENT_CONNECTION_ERRORS => e
      raise BaseClient::Error.new(code: "ENGINE_UNAVAILABLE", message: e.message, status: 503)
    end

    private
      attr_reader :base_url, :signer, :timeout, :open_timeout, :max_get_retries, :http_client, :logger

      def request_json(request_class, path, payload, retryable:)
        body = payload.nil? ? "" : JSON.generate(payload)
        uri = URI.join(base_url, path)
        request = request_class.new(uri)
        request["Accept"] = "application/json"
        request["Content-Type"] = "application/json" unless payload.nil?
        request["X-API-Key"] = ENV["USAGI_API_KEY"] if ENV["USAGI_API_KEY"].present?
        request["Authorization"] = "Bearer #{ENV["USAGI_API_BEARER_TOKEN"]}" if ENV["USAGI_API_BEARER_TOKEN"].present?
        signer.headers(
          method: request.method,
          path_with_query: uri.request_uri,
          body: body,
          request_id: Current.request_id || SecureRandom.uuid
        ).each { |key, value| request[key] = value }
        request.body = body unless payload.nil?

        response = perform_request(uri, request, retryable: retryable)
        parsed = response.body.present? ? JSON.parse(response.body) : {}
        raise BaseClient::Error.from_envelope(parsed, status: response.code.to_i) unless response.is_a?(Net::HTTPSuccess)

        parsed
      rescue JSON::ParserError => e
        raise BaseClient::Error.new(code: "INTERNAL_ERROR", message: "Invalid engine JSON: #{e.message}", status: 502)
      rescue *TRANSIENT_CONNECTION_ERRORS => e
        raise BaseClient::Error.new(code: "ENGINE_UNAVAILABLE", message: e.message, status: 503)
      end

      def perform_request(uri, request, retryable:)
        attempt = 0
        begin
          attempt += 1
          started_at = Process.clock_gettime(Process::CLOCK_MONOTONIC)
          response = http_client.start(uri.host, uri.port, use_ssl: uri.scheme == "https",
                                       read_timeout: timeout, open_timeout: open_timeout) do |http|
            http.request(request)
          end
          log_request(request, uri, response: response, attempt: attempt, duration_ms: duration_since(started_at))
          response
        rescue *TRANSIENT_CONNECTION_ERRORS => e
          if retryable && attempt <= max_get_retries
            log_retry(request, uri, error: e, attempt: attempt)
            retry
          end

          raise
        end
      end

      def log_request(request, uri, response:, attempt:, duration_ms:)
        logger.info(
          event: "engine.request",
          method: request.method,
          path: uri.request_uri,
          status: response.code.to_i,
          request_id: request["X-Request-Id"],
          attempt: attempt,
          duration_ms: duration_ms
        )
      end

      def log_retry(request, uri, error:, attempt:)
        logger.warn(
          event: "engine.request.retry",
          method: request.method,
          path: uri.request_uri,
          request_id: request["X-Request-Id"],
          attempt: attempt,
          next_attempt: attempt + 1,
          error_class: error.class.name,
          error_message: error.message
        )
      end

      def duration_since(started_at)
        ((Process.clock_gettime(Process::CLOCK_MONOTONIC) - started_at) * 1000).round(1)
      end
  end
end
