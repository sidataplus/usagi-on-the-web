# This file is auto-generated from the current state of the database. Instead
# of editing this file, please use the migrations feature of Active Record to
# incrementally modify your database, and then regenerate this schema definition.
#
# This file is the source Rails uses to define your schema when running `bin/rails
# db:schema:load`. When creating a new database, `bin/rails db:schema:load` tends to
# be faster and is potentially less error prone than running all of your
# migrations from scratch. Old migrations may fail to apply correctly if those
# migrations use external dependencies or application code.
#
# It's strongly recommended that you check this file into your version control system.

ActiveRecord::Schema[8.1].define(version: 2026_05_31_120000) do
  create_table "active_storage_attachments", force: :cascade do |t|
    t.bigint "blob_id", null: false
    t.datetime "created_at", null: false
    t.string "name", null: false
    t.string "record_id", null: false
    t.string "record_type", null: false
    t.index ["record_type", "record_id", "name", "blob_id"], name: "index_active_storage_attachments_uniqueness", unique: true
  end

  create_table "active_storage_blobs", force: :cascade do |t|
    t.bigint "byte_size", null: false
    t.string "checksum"
    t.string "content_type"
    t.datetime "created_at", null: false
    t.string "filename", null: false
    t.string "key", null: false
    t.text "metadata"
    t.string "service_name", null: false
    t.index ["key"], name: "index_active_storage_blobs_on_key", unique: true
  end

  create_table "active_storage_variant_records", force: :cascade do |t|
    t.bigint "blob_id", null: false
    t.string "variation_digest", null: false
    t.index ["blob_id", "variation_digest"], name: "index_active_storage_variant_records_uniqueness", unique: true
  end

  create_table "audit_events", id: :string, force: :cascade do |t|
    t.string "action", null: false
    t.datetime "created_at", null: false
    t.json "metadata", default: {}, null: false
    t.string "project_id"
    t.string "request_id"
    t.string "subject_id"
    t.string "subject_type"
    t.datetime "updated_at", null: false
    t.string "user_id"
    t.index ["project_id", "created_at"], name: "index_audit_events_on_project_id_and_created_at"
    t.index ["project_id"], name: "index_audit_events_on_project_id"
    t.index ["request_id"], name: "index_audit_events_on_request_id"
    t.index ["subject_type", "subject_id"], name: "index_audit_events_on_subject_type_and_subject_id"
  end

  create_table "comments", id: :string, force: :cascade do |t|
    t.text "content", null: false
    t.datetime "created_at", null: false
    t.string "mapping_id", null: false
    t.string "parent_id"
    t.string "project_id", null: false
    t.datetime "updated_at", null: false
    t.string "user_id"
    t.index ["mapping_id"], name: "index_comments_on_mapping_id"
    t.index ["parent_id"], name: "index_comments_on_parent_id"
    t.index ["project_id"], name: "index_comments_on_project_id"
  end

  create_table "engine_jobs", id: :string, force: :cascade do |t|
    t.string "api_job_id"
    t.string "candidate_set_id"
    t.datetime "created_at", null: false
    t.json "error", default: {}, null: false
    t.integer "failed", default: 0, null: false
    t.datetime "finished_at"
    t.json "input", default: {}, null: false
    t.string "kind", null: false
    t.string "mode"
    t.integer "processed", default: 0, null: false
    t.string "project_id"
    t.string "requested_by_id"
    t.json "result", default: {}, null: false
    t.string "result_url"
    t.string "stage"
    t.datetime "started_at"
    t.string "state", default: "queued", null: false
    t.string "status_url"
    t.integer "total", default: 0, null: false
    t.datetime "updated_at", null: false
    t.index ["api_job_id"], name: "index_engine_jobs_on_api_job_id", unique: true, where: "api_job_id IS NOT NULL"
    t.index ["project_id", "kind"], name: "index_engine_jobs_on_project_id_and_kind"
    t.index ["project_id"], name: "index_engine_jobs_on_project_id"
    t.index ["requested_by_id"], name: "index_engine_jobs_on_requested_by_id"
    t.index ["state"], name: "index_engine_jobs_on_state"
  end

  create_table "exports", id: :string, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.json "error", default: {}, null: false
    t.string "file_key"
    t.json "filters", default: {}, null: false
    t.datetime "finished_at"
    t.string "format", null: false
    t.string "project_id", null: false
    t.string "requested_by_id"
    t.integer "row_count", default: 0, null: false
    t.datetime "started_at"
    t.string "state", default: "queued", null: false
    t.datetime "updated_at", null: false
    t.index ["project_id", "state"], name: "index_exports_on_project_id_and_state"
    t.index ["project_id"], name: "index_exports_on_project_id"
  end

  create_table "import_sessions", id: :string, force: :cascade do |t|
    t.json "column_mapping", default: {}, null: false
    t.datetime "created_at", null: false
    t.string "created_by_id", null: false
    t.json "detected_columns", default: [], null: false
    t.json "error", default: {}, null: false
    t.string "file_content_type"
    t.string "file_name", null: false
    t.bigint "file_size"
    t.datetime "finished_at"
    t.string "project_id", null: false
    t.text "rows_text"
    t.datetime "started_at"
    t.string "state", default: "pending", null: false
    t.json "summary", default: {}, null: false
    t.datetime "updated_at", null: false
    t.index ["project_id"], name: "index_import_sessions_on_project_id"
    t.index ["state"], name: "index_import_sessions_on_state"
  end

  create_table "mapping_candidates", id: :string, force: :cascade do |t|
    t.float "bimaxsim_score"
    t.string "candidate_set_id"
    t.json "component_ranks", default: {}, null: false
    t.string "concept_class_id"
    t.string "concept_code"
    t.integer "concept_id", null: false
    t.text "concept_name", null: false
    t.datetime "created_at", null: false
    t.string "domain_id"
    t.string "engine_job_id"
    t.json "features", default: {}, null: false
    t.float "final_score"
    t.string "mapping_id", null: false
    t.string "method", null: false
    t.string "project_id", null: false
    t.json "provenance", default: {}, null: false
    t.integer "rank", null: false
    t.float "rrf_score"
    t.float "sapbert_score"
    t.boolean "selected", default: false, null: false
    t.string "source_term_id", null: false
    t.string "standard_concept"
    t.float "tachiom_maxsim_score"
    t.float "tantivy_score"
    t.float "tie_breaker_score"
    t.datetime "updated_at", null: false
    t.string "vocabulary_id"
    t.json "warnings", default: [], null: false
    t.index ["engine_job_id"], name: "index_mapping_candidates_on_engine_job_id"
    t.index ["mapping_id", "concept_id", "method"], name: "idx_candidates_unique_method", unique: true
    t.index ["mapping_id", "rank"], name: "index_mapping_candidates_on_mapping_id_and_rank"
    t.index ["mapping_id"], name: "index_mapping_candidates_on_mapping_id"
    t.index ["project_id", "candidate_set_id"], name: "index_mapping_candidates_on_project_id_and_candidate_set_id"
    t.index ["project_id"], name: "index_mapping_candidates_on_project_id"
  end

  create_table "mappings", id: :string, force: :cascade do |t|
    t.integer "candidate_count", default: 0, null: false
    t.datetime "created_at", null: false
    t.string "equivalence"
    t.integer "lock_version", default: 0, null: false
    t.string "mapping_status", default: "UNCHECKED", null: false
    t.float "match_score"
    t.string "project_id", null: false
    t.datetime "reviewed_at"
    t.string "reviewed_by_id"
    t.string "search_method"
    t.string "source_term_id", null: false
    t.string "target_concept_class_id"
    t.string "target_concept_code"
    t.integer "target_concept_id"
    t.text "target_concept_name"
    t.string "target_domain_id"
    t.string "target_standard_concept"
    t.string "target_vocabulary_id"
    t.datetime "updated_at", null: false
    t.index ["mapping_status"], name: "index_mappings_on_mapping_status"
    t.index ["project_id", "mapping_status"], name: "index_mappings_on_project_id_and_mapping_status"
    t.index ["project_id"], name: "index_mappings_on_project_id"
    t.index ["source_term_id"], name: "index_mappings_on_source_term_id", unique: true
    t.index ["target_concept_id"], name: "index_mappings_on_target_concept_id"
  end

  create_table "project_members", id: :string, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "project_id", null: false
    t.string "role", null: false
    t.datetime "updated_at", null: false
    t.string "user_id", null: false
    t.index ["project_id", "user_id"], name: "index_project_members_on_project_id_and_user_id", unique: true
    t.index ["user_id"], name: "index_project_members_on_user_id"
  end

  create_table "projects", id: :string, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "created_by_id", null: false
    t.text "description"
    t.string "mapping_domain", default: "Drug", null: false
    t.string "name", null: false
    t.json "settings", default: {}, null: false
    t.string "source_vocabulary"
    t.string "status", default: "active", null: false
    t.json "target_domain_ids", default: [], null: false
    t.json "target_vocabulary_ids", default: [], null: false
    t.datetime "updated_at", null: false
    t.string "vocabulary_version"
    t.index ["created_by_id"], name: "index_projects_on_created_by_id"
    t.index ["mapping_domain"], name: "index_projects_on_mapping_domain"
    t.index ["status"], name: "index_projects_on_status"
  end

  create_table "source_terms", id: :string, force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "import_session_id"
    t.text "normalized_source_name"
    t.string "project_id", null: false
    t.json "raw_row", default: {}, null: false
    t.string "source_code", null: false
    t.string "source_domain_hint"
    t.integer "source_frequency", default: 0, null: false
    t.text "source_name", null: false
    t.integer "source_row_number"
    t.string "source_vocabulary"
    t.datetime "updated_at", null: false
    t.index ["project_id", "source_code"], name: "index_source_terms_on_project_id_and_source_code", unique: true
    t.index ["project_id"], name: "index_source_terms_on_project_id"
    t.index ["source_domain_hint"], name: "index_source_terms_on_source_domain_hint"
  end

  create_table "users", id: :string, force: :cascade do |t|
    t.boolean "admin", default: false, null: false
    t.datetime "created_at", null: false
    t.string "email", null: false
    t.datetime "last_seen_at"
    t.string "name", null: false
    t.string "password_digest"
    t.json "preferences", default: {}, null: false
    t.datetime "updated_at", null: false
    t.index ["email"], name: "index_users_on_email", unique: true
  end

  add_foreign_key "active_storage_attachments", "active_storage_blobs", column: "blob_id"
  add_foreign_key "active_storage_variant_records", "active_storage_blobs", column: "blob_id"
  add_foreign_key "audit_events", "projects"
  add_foreign_key "audit_events", "users"
  add_foreign_key "comments", "comments", column: "parent_id"
  add_foreign_key "comments", "mappings"
  add_foreign_key "comments", "projects"
  add_foreign_key "comments", "users"
  add_foreign_key "engine_jobs", "projects"
  add_foreign_key "engine_jobs", "users", column: "requested_by_id"
  add_foreign_key "exports", "projects"
  add_foreign_key "exports", "users", column: "requested_by_id"
  add_foreign_key "import_sessions", "projects"
  add_foreign_key "import_sessions", "users", column: "created_by_id"
  add_foreign_key "mapping_candidates", "engine_jobs"
  add_foreign_key "mapping_candidates", "mappings"
  add_foreign_key "mapping_candidates", "projects"
  add_foreign_key "mapping_candidates", "source_terms"
  add_foreign_key "mappings", "projects"
  add_foreign_key "mappings", "source_terms"
  add_foreign_key "mappings", "users", column: "reviewed_by_id"
  add_foreign_key "project_members", "projects"
  add_foreign_key "project_members", "users"
  add_foreign_key "projects", "users", column: "created_by_id"
  add_foreign_key "source_terms", "import_sessions"
  add_foreign_key "source_terms", "projects"
end
