require "test_helper"

class ExportsBuilderTest < ActiveSupport::TestCase
  setup do
    @user = create_user!(email: "exports@example.test")
    @project = create_project!(user: @user)
    @approved = create_mapping!(
      project: @project,
      source_code: "SRC_APPROVED",
      source_name: "tramadol hcl 50mg cap"
    )
    @approved.update!(
      status: "approved",
      target_concept_id: 40162522,
      target_concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      target_vocabulary_id: "RxNorm",
      target_concept_code: "859751",
      equivalence: "Equivalent"
    )
    @unchecked = create_mapping!(project: @project, source_code: "SRC_UNCHECKED", source_name: "unreviewed drug")
    @approved.mapping_candidates.create!(
      project: @project,
      source_term: @approved.source_term,
      rank: 1,
      concept_id: 40162522,
      concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      method: "hybrid_rrf",
      final_score: 0.93,
      provenance: { "catalog_artifact_id" => "local-catalog-standard-v1" }
    )
  end

  test "source to concept map export includes approved mappings only with STCM columns" do
    csv = CSV.parse(Exports::CsvBuilder.new(@project, format: "source_to_concept_map_csv").to_csv, headers: true)

    assert_equal ["source_code", "source_name", "source_vocabulary_id", "target_concept_id", "target_concept_name", "target_vocabulary_id", "target_concept_code", "equivalence"], csv.headers
    assert_equal ["SRC_APPROVED"], csv.map { |row| row.fetch("source_code") }
    assert_equal "859751", csv.first.fetch("target_concept_code")
  end

  test "candidate jsonl export includes one JSON object per candidate with provenance" do
    lines = Exports::CsvBuilder.new(@project, format: "candidate_jsonl").to_jsonl.lines.map { |line| JSON.parse(line) }

    assert_equal 1, lines.size
    assert_equal "SRC_APPROVED", lines.first.fetch("source_code")
    assert_equal 40162522, lines.first.fetch("concept_id")
    assert_equal "local-catalog-standard-v1", lines.first.fetch("provenance").fetch("catalog_artifact_id")
  end
end
