module Mappings
  class HybridSearchBatchEnumerator
    def initialize(relation:, batch_size:, cursor: %i[created_at id], order: %i[asc asc])
      @relation = relation
      @batch_size = batch_size
      @cursor = cursor
      @order = order
    end

    def each_slice
      return enum_for(:each_slice) unless block_given?

      slice = []
      relation.find_each(batch_size: batch_size, cursor: cursor, order: order) do |mapping|
        slice << mapping
        next if slice.size < batch_size

        yield slice
        slice = []
      end
      yield slice if slice.any?
    end

    private
      attr_reader :relation, :batch_size, :cursor, :order
  end
end
