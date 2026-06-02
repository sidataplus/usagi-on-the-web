require "json"
require "net/http"
require "securerandom"
require "uri"

module EngineClients
  class HttpTransport
    def initialize(base_url:, signer: RequestSigner.new, timeout: ENV.fetch("ENGINE_API_TIMEOUT_SECONDS", 30).to_f,
                   open_timeout: ENV.fetch("ENGINE_API_OPEN_TIMEOUT_SECONDS", 5).to_f)
      @base_url = base_url
      @signer = signer
      @timeout = timeout
      @open_timeout = open_timeout
    end

    def get_json(path)
      request_json(Net::HTTP::Get, path, nil)
    end

    def post_json(path, payload)
      request_json(Net::HTTP::Post, path, payload)
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

      response = Net::HTTP.start(uri.host, uri.port, use_ssl: uri.scheme == "https",
                                 read_timeout: timeout, open_timeout: open_timeout) do |http|
        http.request(request)
      end
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
    rescue Timeout::Error, Errno::ECONNREFUSED, SocketError => e
      raise BaseClient::Error.new(code: "ENGINE_UNAVAILABLE", message: e.message, status: 503)
    end

    private
      attr_reader :base_url, :signer, :timeout, :open_timeout

      def request_json(request_class, path, payload)
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

        response = Net::HTTP.start(uri.host, uri.port, use_ssl: uri.scheme == "https",
                                   read_timeout: timeout, open_timeout: open_timeout) do |http|
          http.request(request)
        end
        parsed = response.body.present? ? JSON.parse(response.body) : {}
        raise BaseClient::Error.from_envelope(parsed, status: response.code.to_i) unless response.is_a?(Net::HTTPSuccess)

        parsed
      rescue JSON::ParserError => e
        raise BaseClient::Error.new(code: "INTERNAL_ERROR", message: "Invalid engine JSON: #{e.message}", status: 502)
      rescue Timeout::Error, Errno::ECONNREFUSED, SocketError => e
        raise BaseClient::Error.new(code: "ENGINE_UNAVAILABLE", message: e.message, status: 503)
      end
  end
end
