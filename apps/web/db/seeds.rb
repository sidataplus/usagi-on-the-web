# frozen_string_literal: true

def seed_demo_project!(owner:, include_reviewer: true)
  project = Project.find_or_initialize_by(name: "Demo: Augmentin Drug Review")
  project.update!(
    description: "Fixture-backed drug source strings mapped to OMOP standard RxNorm concepts.",
    created_by: owner,
    mapping_domain: "Drug",
    source_vocabulary: "NDC",
    target_vocabulary_ids: ["RxNorm"],
    vocabulary_version: "dev",
    status: "active"
  )

  if include_reviewer
    reviewer = User.find_or_initialize_by(email: "reviewer@usagi.test")
    reviewer.update!(name: "Review User", password: "password123", password_confirmation: "password123")
    project.project_members.find_or_create_by!(user: reviewer) { |member| member.role = "reviewer" }
  end

  import = project.import_sessions.find_or_create_by!(file_name: "demo-drugs.csv", created_by: owner) do |session|
    session.state = "succeeded"
    session.summary = { rows_seen: 1, rows_imported: 1, rows_failed: 0 }
    session.finished_at = Time.current
  end

  [
    ["SRC_AUGMENTIN_875_125", "Augmentin 875/125", 12]
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

  project
end

if LocalDeployAuth.enabled?
  owner = LocalDeployAuth.ensure_user!
  seed_demo_project!(owner: owner, include_reviewer: false)
  puts "Seeded local deployment workspace for #{owner.email} (passwordless sign-in)"
else
  demo = User.find_or_initialize_by(email: "demo@usagi.test")
  demo.update!(name: "Demo Reviewer", password: "password123", password_confirmation: "password123", admin: true)
  seed_demo_project!(owner: demo, include_reviewer: true)
  puts "Seeded demo account demo@usagi.test / password123"
end
