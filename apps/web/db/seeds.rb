# frozen_string_literal: true

demo = User.find_or_initialize_by(email: "demo@usagi.test")
demo.update!(name: "Demo Reviewer", password: "password123", password_confirmation: "password123", admin: true)

reviewer = User.find_or_initialize_by(email: "reviewer@usagi.test")
reviewer.update!(name: "Review User", password: "password123", password_confirmation: "password123")

project = Project.find_or_initialize_by(name: "Pharmacy NDC to RxNorm")
project.update!(
  description: "Drug source strings mapped to OMOP standard RxNorm concepts.",
  created_by: demo,
  mapping_domain: "Drug",
  source_vocabulary: "NDC",
  target_vocabulary_ids: ["RxNorm"],
  vocabulary_version: "dev",
  status: "active"
)
project.project_members.find_or_create_by!(user: reviewer) { |member| member.role = "reviewer" }

import = project.import_sessions.find_or_create_by!(file_name: "demo-drugs.csv", created_by: demo) do |session|
  session.state = "succeeded"
  session.summary = { rows_seen: 2, rows_imported: 2, rows_failed: 0 }
  session.finished_at = Time.current
end

[
  ["SRC_TRAMADOL", "tramadol hcl 50mg cap", 12],
  ["SRC_METFORMIN", "metformin hcl 500 mg tab", 7]
].each_with_index do |(code, name, frequency), index|
  term = project.source_terms.find_or_create_by!(source_code: code) do |source_term|
    source_term.import_session = import
    source_term.source_name = name
    source_term.source_frequency = frequency
    source_term.source_row_number = index + 1
    source_term.source_vocabulary = "NDC"
  end
  project.mappings.find_or_create_by!(source_term: term) do |mapping|
    mapping.mapping_status = "UNCHECKED"
  end
end

puts "Seeded demo account demo@usagi.test / password123"
