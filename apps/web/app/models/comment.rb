class Comment < ApplicationRecord
  include HasPrefixedId

  prefixed_id "com"

  belongs_to :project
  belongs_to :mapping
  belongs_to :user, optional: true
  belongs_to :parent, class_name: "Comment", optional: true
  has_many :replies, class_name: "Comment", foreign_key: :parent_id,
                     inverse_of: :parent, dependent: :destroy

  validates :content, presence: true

  scope :roots, -> { where(parent_id: nil) }
  scope :chronological, -> { order(created_at: :asc) }

  def author_name
    user&.display_name || "Unknown"
  end
end
