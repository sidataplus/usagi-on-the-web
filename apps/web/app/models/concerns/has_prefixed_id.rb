module HasPrefixedId
  extend ActiveSupport::Concern

  included do
    before_validation :assign_prefixed_id, on: :create
  end

  class_methods do
    def prefixed_id(prefix)
      @prefixed_id_prefix = prefix
    end

    def prefixed_id_prefix
      @prefixed_id_prefix || model_name.singular
    end
  end

  private
    def assign_prefixed_id
      self.id ||= Ids.generate(self.class.prefixed_id_prefix)
    end
end
