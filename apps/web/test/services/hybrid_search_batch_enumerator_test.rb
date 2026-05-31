require "test_helper"

class HybridSearchBatchEnumeratorTest < ActiveSupport::TestCase
  FakeMapping = Struct.new(:source_code)

  class LargeRelation
    attr_reader :batch_sizes

    def initialize(size)
      @size = size
      @batch_sizes = []
    end

    def find_each(batch_size:, cursor: nil, order: nil)
      @batch_sizes << batch_size
      return enum_for(:find_each, batch_size: batch_size, cursor: cursor, order: order) unless block_given?

      @size.times { |index| yield FakeMapping.new("SRC_#{index}") }
    end

    def to_a
      raise "relation should not be materialized"
    end
  end

  test "streams a 100k mapping path in bounded slices" do
    relation = LargeRelation.new(100_000)
    slices = []

    Mappings::HybridSearchBatchEnumerator.new(relation: relation, batch_size: 1_000).each_slice do |slice|
      slices << slice.size
    end

    assert_equal [1_000], relation.batch_sizes
    assert_equal 100, slices.size
    assert_equal 100_000, slices.sum
    assert_equal [1_000], slices.uniq
  end
end
