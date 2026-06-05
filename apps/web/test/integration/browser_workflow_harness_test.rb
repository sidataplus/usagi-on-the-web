require "test_helper"

class BrowserWorkflowHarnessTest < ActiveSupport::TestCase
  ROOT = Rails.root.join("../..").expand_path
  HARNESS_PATH = ROOT.join("scripts/browser-live-workflow.mjs")
  VERIFICATION_DOC = ROOT.join("docs/manual/web/03-verification.md")

  test "browser live workflow harness is documented and names the fixture-backed terms" do
    assert_path_exists HARNESS_PATH

    harness = HARNESS_PATH.read
    docs = VERIFICATION_DOC.read

    assert_includes harness, "runLiveBrowserWorkflow"
    assert_includes harness, "LIVE_BROWSER_WORKFLOW_TERMS"
    assert_includes harness, 'baseUrl: "http://127.0.0.1:3000"'
    assert_includes harness, "fillWithTypingFallback"
    assert_includes harness, "clickByTextCenter"
    assert_includes harness, "Augmentin 875/125"
    assert_includes harness, "query with no precomputed embedding"
    assert_includes harness, "amoxicillin 875 MG / clavulanate 125 MG Oral Tablet"

    assert_includes docs, "scripts/browser-live-workflow.mjs"
    assert_includes docs, "runLiveBrowserWorkflow"
  end
end
