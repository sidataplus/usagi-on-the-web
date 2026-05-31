module EngineClients
  # Shared base for Rails-side adapters to the Rust engine services. The default
  # transport is in-process while the API endpoints are still tightening; callers
  # should depend on these clients instead of reaching into UsagiApi::Stub.
  class BaseClient
    class Error < StandardError
      attr_reader :code, :details, :request_id, :status

      def initialize(code:, message:, details: {}, request_id: nil, status: nil)
        super(message)
        @code = code
        @details = details || {}
        @request_id = request_id
        @status = status
      end

      def self.from_envelope(envelope, status: nil)
        error = envelope.fetch("error", {})
        new(
          code: error.fetch("code", "INTERNAL_ERROR"),
          message: error.fetch("message", "Engine request failed"),
          details: error["details"] || {},
          request_id: error["request_id"],
          status: status
        )
      end
    end

    class << self
      attr_writer :default_transport

      def default_transport
        @default_transport ||= ENV.fetch("ENGINE_CLIENT_MODE", "stub") == "http" ? http_transport : StubTransport.new
      end

      def http_transport
        raise NotImplementedError, "configure transport in concrete client"
      end
    end

    def initialize(transport: self.class.default_transport)
      @transport = transport
    end

    private
      attr_reader :transport

      def concept_result_from_payload(result, provenance: {})
        concept = result.fetch("concept")
        scores = result["scores"] || {}
        UsagiApi::ConceptResult.from_concept(
          {
            concept_id: concept["concept_id"],
            concept_name: concept["concept_name"],
            domain_id: concept["domain_id"],
            vocabulary_id: concept["vocabulary_id"],
            concept_class_id: concept["concept_class_id"],
            standard_concept: concept["standard_concept"],
            concept_code: concept["concept_code"]
          },
          score: scores["final"] || scores["rrf"] || scores["bimaxsim"] || scores["tachiom_maxsim"] || scores["sapbert"] || scores["tantivy"],
          method: result["method"],
          rank: result["rank"]
        ).with(provenance: provenance, scores: scores, features: result["features"] || {}, warnings: result["warnings"] || [])
      end
  end
end
