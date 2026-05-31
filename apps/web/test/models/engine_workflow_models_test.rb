require "test_helper"

class EngineWorkflowModelsTest < ActiveSupport::TestCase
  setup do
    @user = create_user!(email: "owner@example.test")
    @project = create_project!(user: @user)
  end

  test "core records use target prefixed string ids" do
    mapping = create_mapping!(project: @project)
    candidate = mapping.mapping_candidates.create!(
      project: @project,
      source_term: mapping.source_term,
      concept_id: 40162522,
      concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      domain_id: "Drug",
      vocabulary_id: "RxNorm",
      rank: 1,
      final_score: 0.91,
      method: "thirawat_tachiom_bimaxsim_tiebreak"
    )

    assert_match(/\Ausr_/, @user.id)
    assert_match(/\Aproj_/, @project.id)
    assert_match(/\Apmem_/, @project.project_members.first.id)
    assert_match(/\Amap_/, mapping.id)
    assert_match(/\Asrc_/, mapping.source_term.id)
    assert_match(/\Acand_/, candidate.id)
  end

  test "project membership permissions follow MVP roles" do
    reviewer = create_user!(email: "reviewer@example.test")
    member = @project.project_members.create!(user: reviewer, role: "reviewer")

    assert member.can_review?
    assert member.can_run_suggestions?
    assert member.can_export?
    assert_not member.can_import?
  end

  test "candidate application does not auto approve mappings" do
    mapping = create_mapping!(project: @project)
    candidate = mapping.mapping_candidates.create!(
      project: @project,
      source_term: mapping.source_term,
      concept_id: 40162522,
      concept_name: "tramadol hydrochloride 50 MG Oral Capsule",
      rank: 1,
      final_score: 0.91,
      method: "thirawat_tachiom_bimaxsim_tiebreak"
    )

    Mappings::CandidatePersister.new(mapping: mapping).persist_results([
      UsagiApi::ConceptResult.from_concept(
        {
          concept_id: candidate.concept_id,
          concept_name: candidate.concept_name,
          domain_id: "Drug",
          vocabulary_id: "RxNorm",
          concept_class_id: "Clinical Drug",
          standard_concept: "S",
          concept_code: "859751"
        },
        score: 0.92,
        method: candidate.method,
        rank: 1
      )
    ])

    assert_equal "UNCHECKED", mapping.reload.mapping_status
    assert_equal 1, mapping.mapping_candidates.count
  end

  test "request signer emits canonical signed headers" do
    headers = EngineClients::RequestSigner.new(secret: "secret").headers(
      method: "POST",
      path_with_query: "/search/concepts",
      body: "{\"q\":\"tramadol\"}",
      request_id: "req_test"
    )

    assert_equal "req_test", headers.fetch("X-Request-Id")
    assert_equal "rails", headers.fetch("X-Usagi-Service")
    assert_match(/\Av1=/, headers.fetch("X-Usagi-Signature"))
    assert headers.fetch("X-Usagi-Content-SHA256").present?
  end
end
