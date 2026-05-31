require "test_helper"

class ExportJobsTest < ActiveJob::TestCase
  setup do
    @user = create_user!(email: "export-job@example.test")
    @project = create_project!(user: @user)
    @mapping = create_mapping!(project: @project)
    create_mapping!(project: @project, source_code: "SRC_NO_CANDIDATE", source_name: "plain row")
    @mapping.mapping_candidates.create!(
      project: @project,
      source_term: @mapping.source_term,
      rank: 1,
      concept_id: 40162522,
      concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      method: "hybrid_rrf",
      final_score: 0.91,
      provenance: { "search_artifact_id" => "search-v1" }
    )
  end

  test "candidate jsonl export job writes ndjson artifact and format row count" do
    export = @project.exports.create!(
      requested_by: @user,
      format: "candidate_jsonl",
      state: "queued",
      file_key: "candidate-export.jsonl"
    )

    CreateExportJob.perform_now(export.id)

    assert_equal "succeeded", export.reload.state
    assert_equal 1, export.row_count
    assert export.artifact.attached?
    assert_equal "candidate-export.jsonl", export.artifact.filename.to_s
    assert_equal "application/x-ndjson", export.artifact.content_type
    assert_equal "search-v1", JSON.parse(export.artifact.download.lines.first).fetch("provenance").fetch("search_artifact_id")
  end
end
