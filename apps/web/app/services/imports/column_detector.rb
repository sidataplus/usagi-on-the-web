module Imports
  class ColumnDetector
    SOURCE_CODE = /\A(source_)?(code|concept_code|id)\z/i
    SOURCE_NAME = /\A(source_)?(name|description|term|concept_name)\z/i
    FREQUENCY = /\A(source_)?(frequency|freq|count|n)\z/i
    DOMAIN_HINT = /\A(source_)?(domain|domain_hint)\z/i

    def self.detect(headers)
      normalized = Array(headers).map(&:to_s)
      {
        "source_code" => find_header(normalized, SOURCE_CODE),
        "source_name" => find_header(normalized, SOURCE_NAME),
        "source_frequency" => find_header(normalized, FREQUENCY),
        "source_domain_hint" => find_header(normalized, DOMAIN_HINT)
      }.compact
    end

    def self.find_header(headers, pattern)
      headers.find { |header| header.strip.match?(pattern) }
    end
    private_class_method :find_header
  end
end
