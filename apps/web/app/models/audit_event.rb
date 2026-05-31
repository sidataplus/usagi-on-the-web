class AuditEvent < ApplicationRecord
  include HasPrefixedId

  prefixed_id "aud"

  belongs_to :project, optional: true
  belongs_to :user, optional: true
  belongs_to :subject, polymorphic: true, optional: true

  validates :action, presence: true

  scope :recent_first, -> { order(created_at: :desc, id: :desc) }

  def author_name
    user&.display_name || "System"
  end
end
