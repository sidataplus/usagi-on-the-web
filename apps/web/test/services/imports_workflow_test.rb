require "test_helper"

class ImportsWorkflowTest < ActiveSupport::TestCase
  setup do
    @user = create_user!(email: "imports@example.test")
    @project = create_project!(user: @user)
  end

  test "column detector recognizes common source term headers" do
    mapping = Imports::ColumnDetector.detect(%w[Code Description Count Domain])

    assert_equal "Code", mapping.fetch("source_code")
    assert_equal "Description", mapping.fetch("source_name")
    assert_equal "Count", mapping.fetch("source_frequency")
    assert_equal "Domain", mapping.fetch("source_domain_hint")
  end

  test "source term importer creates valid rows and reports row level errors" do
    import = @project.import_sessions.create!(created_by: @user, file_name: "row-errors.csv", state: "pending")
    rows = [
      { source_code: "DUP", source_name: "metformin hcl 500 mg tab", source_frequency: 10, source_row_number: 1, raw_row: { "source_code" => "DUP" } },
      { source_code: "DUP", source_name: "duplicate metformin", source_frequency: 8, source_row_number: 2, raw_row: { "source_code" => "DUP" } },
      { source_code: "BLANK", source_name: "", source_frequency: 3, source_row_number: 3, raw_row: { "source_code" => "BLANK" } },
      { source_code: "OK", source_name: "tramadol hcl 50mg cap", source_frequency: 5, source_row_number: 4, raw_row: { "source_code" => "OK" } }
    ]

    result = Imports::SourceTermImporter.new(project: @project, import_session: import).import(rows)

    assert_equal 4, result.rows_seen
    assert_equal 2, result.rows_imported
    assert_equal 2, result.rows_failed
    assert_equal ["DUP", "OK"], @project.source_terms.order(:source_code).pluck(:source_code)
    assert_equal 2, @project.mappings.count
    assert_equal [2, 3], result.errors.map { |error| error.fetch(:row_number) }
    assert_match(/duplicate/i, result.errors.first.fetch(:message))
  end
end
