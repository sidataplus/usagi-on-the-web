class CreateUsagiProductMvp < ActiveRecord::Migration[8.1]
  def change
    create_table :users, id: :string do |t|
      t.string :email, null: false
      t.string :name, null: false
      t.string :password_digest
      t.boolean :admin, null: false, default: false
      t.datetime :last_seen_at
      t.json :preferences, null: false, default: {}
      t.timestamps
    end
    add_index :users, :email, unique: true

    create_table :projects, id: :string do |t|
      t.string :name, null: false
      t.text :description
      t.string :status, null: false, default: "active"
      t.string :mapping_domain, null: false, default: "Drug"
      t.string :source_vocabulary
      t.string :vocabulary_version
      t.json :target_domain_ids, null: false, default: []
      t.json :target_vocabulary_ids, null: false, default: []
      t.json :settings, null: false, default: {}
      t.string :created_by_id, null: false
      t.timestamps
    end
    add_index :projects, :status
    add_index :projects, :mapping_domain
    add_index :projects, :created_by_id

    create_table :project_members, id: :string do |t|
      t.string :project_id, null: false
      t.string :user_id, null: false
      t.string :role, null: false
      t.timestamps
    end
    add_index :project_members, [:project_id, :user_id], unique: true
    add_index :project_members, :user_id

    create_table :import_sessions, id: :string do |t|
      t.string :project_id, null: false
      t.string :created_by_id, null: false
      t.string :file_name, null: false
      t.string :file_content_type
      t.bigint :file_size
      t.string :state, null: false, default: "pending"
      t.json :column_mapping, null: false, default: {}
      t.json :detected_columns, null: false, default: []
      t.json :summary, null: false, default: {}
      t.json :error, null: false, default: {}
      t.text :rows_text
      t.datetime :started_at
      t.datetime :finished_at
      t.timestamps
    end
    add_index :import_sessions, :project_id
    add_index :import_sessions, :state

    create_table :source_terms, id: :string do |t|
      t.string :project_id, null: false
      t.string :import_session_id
      t.string :source_code, null: false
      t.text :source_name, null: false
      t.integer :source_frequency, null: false, default: 0
      t.string :source_domain_hint
      t.string :source_vocabulary
      t.integer :source_row_number
      t.text :normalized_source_name
      t.json :raw_row, null: false, default: {}
      t.timestamps
    end
    add_index :source_terms, :project_id
    add_index :source_terms, [:project_id, :source_code], unique: true
    add_index :source_terms, :source_domain_hint

    create_table :mappings, id: :string do |t|
      t.string :project_id, null: false
      t.string :source_term_id, null: false
      t.integer :target_concept_id
      t.text :target_concept_name
      t.string :target_domain_id
      t.string :target_vocabulary_id
      t.string :target_concept_class_id
      t.string :target_standard_concept
      t.string :target_concept_code
      t.float :match_score
      t.string :search_method
      t.string :mapping_status, null: false, default: "UNCHECKED"
      t.string :equivalence
      t.string :reviewed_by_id
      t.datetime :reviewed_at
      t.integer :candidate_count, null: false, default: 0
      t.integer :lock_version, null: false, default: 0
      t.timestamps
    end
    add_index :mappings, :project_id
    add_index :mappings, :source_term_id, unique: true
    add_index :mappings, :mapping_status
    add_index :mappings, :target_concept_id
    add_index :mappings, [:project_id, :mapping_status]

    create_table :mapping_candidates, id: :string do |t|
      t.string :project_id, null: false
      t.string :mapping_id, null: false
      t.string :source_term_id, null: false
      t.string :engine_job_id
      t.integer :rank, null: false
      t.integer :concept_id, null: false
      t.text :concept_name, null: false
      t.string :domain_id
      t.string :vocabulary_id
      t.string :concept_class_id
      t.string :standard_concept
      t.string :concept_code
      t.float :tantivy_score
      t.float :sapbert_score
      t.float :rrf_score
      t.float :tachiom_maxsim_score
      t.float :bimaxsim_score
      t.float :tie_breaker_score
      t.float :final_score
      t.string :method, null: false
      t.string :candidate_set_id
      t.json :component_ranks, null: false, default: {}
      t.json :features, null: false, default: {}
      t.json :provenance, null: false, default: {}
      t.json :warnings, null: false, default: []
      t.boolean :selected, null: false, default: false
      t.timestamps
    end
    add_index :mapping_candidates, :project_id
    add_index :mapping_candidates, :mapping_id
    add_index :mapping_candidates, :engine_job_id
    add_index :mapping_candidates, [:mapping_id, :rank]
    add_index :mapping_candidates, [:project_id, :candidate_set_id]
    add_index :mapping_candidates, [:mapping_id, :concept_id, :method],
              unique: true, name: "idx_candidates_unique_method"

    create_table :engine_jobs, id: :string do |t|
      t.string :project_id
      t.string :requested_by_id
      t.string :api_job_id
      t.string :kind, null: false
      t.string :state, null: false, default: "queued"
      t.string :stage
      t.integer :processed, null: false, default: 0
      t.integer :total, null: false, default: 0
      t.integer :failed, null: false, default: 0
      t.string :mode
      t.string :status_url
      t.string :result_url
      t.string :candidate_set_id
      t.json :input, null: false, default: {}
      t.json :result, null: false, default: {}
      t.json :error, null: false, default: {}
      t.datetime :started_at
      t.datetime :finished_at
      t.timestamps
    end
    add_index :engine_jobs, :project_id
    add_index :engine_jobs, :api_job_id, unique: true, where: "api_job_id IS NOT NULL"
    add_index :engine_jobs, [:project_id, :kind]
    add_index :engine_jobs, :state
    add_index :engine_jobs, :requested_by_id

    create_table :exports, id: :string do |t|
      t.string :project_id, null: false
      t.string :requested_by_id
      t.string :format, null: false
      t.string :state, null: false, default: "queued"
      t.integer :row_count, null: false, default: 0
      t.json :filters, null: false, default: {}
      t.string :file_key
      t.json :error, null: false, default: {}
      t.datetime :started_at
      t.datetime :finished_at
      t.timestamps
    end
    add_index :exports, :project_id
    add_index :exports, [:project_id, :state]

    create_table :audit_events, id: :string do |t|
      t.string :project_id
      t.string :user_id
      t.string :subject_type
      t.string :subject_id
      t.string :action, null: false
      t.string :request_id
      t.json :metadata, null: false, default: {}
      t.timestamps
    end
    add_index :audit_events, :project_id
    add_index :audit_events, [:project_id, :created_at]
    add_index :audit_events, [:subject_type, :subject_id]
    add_index :audit_events, :request_id

    create_table :comments, id: :string do |t|
      t.string :project_id, null: false
      t.string :mapping_id, null: false
      t.string :user_id
      t.string :parent_id
      t.text :content, null: false
      t.timestamps
    end
    add_index :comments, :project_id
    add_index :comments, :mapping_id
    add_index :comments, :parent_id

    create_table :active_storage_blobs do |t|
      t.string :key, null: false
      t.string :filename, null: false
      t.string :content_type
      t.text :metadata
      t.string :service_name, null: false
      t.bigint :byte_size, null: false
      t.string :checksum
      t.datetime :created_at, null: false
      t.index :key, unique: true
    end

    create_table :active_storage_attachments do |t|
      t.string :name, null: false
      t.string :record_type, null: false
      t.string :record_id, null: false
      t.bigint :blob_id, null: false
      t.datetime :created_at, null: false
      t.index [:record_type, :record_id, :name, :blob_id],
              unique: true, name: "index_active_storage_attachments_uniqueness"
    end

    create_table :active_storage_variant_records do |t|
      t.bigint :blob_id, null: false
      t.string :variation_digest, null: false
      t.index [:blob_id, :variation_digest],
              unique: true, name: "index_active_storage_variant_records_uniqueness"
    end

    add_foreign_key :projects, :users, column: :created_by_id
    add_foreign_key :project_members, :projects
    add_foreign_key :project_members, :users
    add_foreign_key :import_sessions, :projects
    add_foreign_key :import_sessions, :users, column: :created_by_id
    add_foreign_key :source_terms, :projects
    add_foreign_key :source_terms, :import_sessions
    add_foreign_key :mappings, :projects
    add_foreign_key :mappings, :source_terms
    add_foreign_key :mappings, :users, column: :reviewed_by_id
    add_foreign_key :mapping_candidates, :projects
    add_foreign_key :mapping_candidates, :mappings
    add_foreign_key :mapping_candidates, :source_terms
    add_foreign_key :mapping_candidates, :engine_jobs
    add_foreign_key :engine_jobs, :projects
    add_foreign_key :engine_jobs, :users, column: :requested_by_id
    add_foreign_key :exports, :projects
    add_foreign_key :exports, :users, column: :requested_by_id
    add_foreign_key :audit_events, :projects
    add_foreign_key :audit_events, :users
    add_foreign_key :comments, :projects
    add_foreign_key :comments, :mappings
    add_foreign_key :comments, :users
    add_foreign_key :comments, :comments, column: :parent_id
    add_foreign_key :active_storage_attachments, :active_storage_blobs, column: :blob_id
    add_foreign_key :active_storage_variant_records, :active_storage_blobs, column: :blob_id
  end
end
