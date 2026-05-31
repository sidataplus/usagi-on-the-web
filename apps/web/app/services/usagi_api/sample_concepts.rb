module UsagiApi
  # Curated, illustrative OMOP standard concepts used to back the wireframe
  # while the real catalog/search/mapper API (see docs/api/endpoints.md) is not
  # yet wired up. The live catalog is API-owned; these are sample data only and
  # the concept_ids/codes are representative rather than authoritative.
  module SampleConcepts
    CONCEPTS = [
      # --- Conditions (SNOMED) ---
      { concept_id: 201826,  concept_name: "Type 2 diabetes mellitus",       domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "44054006" },
      { concept_id: 316866,  concept_name: "Hypertensive disorder",          domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "38341003" },
      { concept_id: 432867,  concept_name: "Hyperlipidemia",                 domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "55822004" },
      { concept_id: 255848,  concept_name: "Pneumonia",                      domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "233604007" },
      { concept_id: 317009,  concept_name: "Asthma",                         domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "195967001" },
      { concept_id: 313217,  concept_name: "Atrial fibrillation",            domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "49436004" },
      { concept_id: 4329847, concept_name: "Myocardial infarction",          domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "22298006" },
      { concept_id: 440383,  concept_name: "Depressive disorder",            domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "35489007" },
      { concept_id: 378419,  concept_name: "Alzheimer's disease",            domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "26929004" },
      { concept_id: 201254,  concept_name: "Type 1 diabetes mellitus",       domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "46635009" },
      { concept_id: 80180,   concept_name: "Osteoarthritis",                 domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "396275006" },
      { concept_id: 80809,   concept_name: "Rheumatoid arthritis",           domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "69896004" },
      { concept_id: 4182210, concept_name: "Dementia",                       domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "52448006" },
      { concept_id: 372924,  concept_name: "Cerebral infarction",            domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "432504007" },
      { concept_id: 4030518, concept_name: "Chronic kidney disease",         domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "709044004" },
      { concept_id: 254761,  concept_name: "Cough",                          domain_id: "Condition", vocabulary_id: "SNOMED", concept_class_id: "Clinical Finding", standard_concept: "S", concept_code: "49727002" },

      # --- Drugs (RxNorm) ---
      { concept_id: 19059834, concept_name: "metformin hydrochloride 500 MG Oral Tablet",   domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "861007" },
      { concept_id: 19019073, concept_name: "aspirin 81 MG Oral Tablet",                    domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "243670" },
      { concept_id: 19102189, concept_name: "lisinopril 10 MG Oral Tablet",                 domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "314076" },
      { concept_id: 19006318, concept_name: "atorvastatin 20 MG Oral Tablet",               domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "617310" },
      { concept_id: 19003485, concept_name: "amlodipine 5 MG Oral Tablet",                  domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "197361" },
      { concept_id: 40162522, concept_name: "tramadol hydrochloride 50 MG Oral Capsule",    domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "859751" },
      { concept_id: 19074673, concept_name: "omeprazole 20 MG Delayed Release Oral Capsule", domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "198051" },
      { concept_id: 19077524, concept_name: "simvastatin 20 MG Oral Tablet",                domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "312962" },
      { concept_id: 19127663, concept_name: "levothyroxine sodium 0.05 MG Oral Tablet",     domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "966222" },
      { concept_id: 19030765, concept_name: "amoxicillin 500 MG Oral Capsule",              domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "308191" },
      { concept_id: 19009384, concept_name: "hydrochlorothiazide 25 MG Oral Tablet",        domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "310798" },
      { concept_id: 19075601, concept_name: "ibuprofen 200 MG Oral Tablet",                 domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "310965" },
      { concept_id: 19133768, concept_name: "gabapentin 300 MG Oral Capsule",               domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "310431" },
      { concept_id: 19112599, concept_name: "sertraline 50 MG Oral Tablet",                 domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "312940" },
      { concept_id: 19057400, concept_name: "furosemide 40 MG Oral Tablet",                 domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "310429" },
      { concept_id: 19035704, concept_name: "prednisone 10 MG Oral Tablet",                 domain_id: "Drug", vocabulary_id: "RxNorm", concept_class_id: "Clinical Drug", standard_concept: "S", concept_code: "312615" }
    ].freeze

    BY_ID = CONCEPTS.index_by { |concept| concept[:concept_id] }.freeze

    module_function

    def all
      CONCEPTS
    end

    def find(concept_id)
      BY_ID[concept_id.to_i]
    end

    def by_domain(domain)
      CONCEPTS.select { |concept| concept[:domain_id] == domain }
    end

    # Naive substring match over name/code; the real search-api does lexical +
    # dense + hybrid RRF ranking (POST /search/concepts).
    def search(query, limit: 20)
      return CONCEPTS.first(limit) if query.to_s.strip.empty?

      needle = query.to_s.downcase
      CONCEPTS.select do |concept|
        concept[:concept_name].downcase.include?(needle) ||
          concept[:concept_code].include?(needle)
      end.first(limit)
    end
  end
end
