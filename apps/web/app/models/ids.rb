require "securerandom"

module Ids
  PREFIXES = {
    user: "usr",
    project: "proj",
    project_member: "pmem",
    import_session: "imp",
    source_term: "src",
    mapping: "map",
    mapping_candidate: "cand",
    engine_job: "ejob",
    export: "exp",
    audit_event: "aud",
    comment: "com"
  }.freeze

  def self.generate(prefix)
    "#{prefix}_#{SecureRandom.uuid}"
  end
end
