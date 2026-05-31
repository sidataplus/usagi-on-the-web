-- Deterministic 50-drug development fixture extraction.
--
-- Run from the repository root:
-- duckdb /Users/na399/GitHub/omop_vocab_v20260227.duckdb < scripts/dev/extract_dev_drugs_50.sql
--
-- DuckDB is used only as a local development source for fixture extraction.
-- Runtime catalog truth remains SQLite.

CREATE OR REPLACE TEMP TABLE seed_drugs AS
SELECT *
FROM read_csv(
  'fixtures/dev-drugs-50/seeds.csv',
  header = true,
  columns = {'seed_order': 'INTEGER', 'seed_name': 'VARCHAR'}
);

COPY seed_drugs TO 'fixtures/dev-drugs-50/seeds.csv' (HEADER, DELIMITER ',');

CREATE OR REPLACE TEMP TABLE standard_candidates AS
SELECT
  s.seed_order,
  s.seed_name,
  c.concept_id,
  c.concept_name,
  c.domain_id,
  c.vocabulary_id,
  c.concept_class_id,
  c.standard_concept,
  c.concept_code,
  row_number() OVER (
    PARTITION BY s.seed_order
    ORDER BY
      CASE
        WHEN c.concept_class_id IN ('Clinical Drug', 'Clinical Drug Comp') THEN 0
        WHEN c.concept_class_id IN ('Branded Drug', 'Branded Drug Comp') THEN 1
        WHEN c.concept_class_id = 'Ingredient' THEN 2
        ELSE 3
      END,
      CASE WHEN c.vocabulary_id = 'RxNorm' THEN 0 ELSE 1 END,
      length(c.concept_name),
      c.concept_id
  ) AS rn
FROM seed_drugs s
JOIN concept c
  ON lower(c.concept_name) LIKE '%' || lower(s.seed_name) || '%'
WHERE c.domain_id = 'Drug'
  AND c.standard_concept = 'S'
  AND coalesce(c.invalid_reason, '') = ''
  AND c.vocabulary_id IN ('RxNorm', 'RxNorm Extension')
  AND c.concept_class_id IN (
    'Clinical Drug',
    'Clinical Drug Comp',
    'Branded Drug',
    'Branded Drug Comp',
    'Ingredient'
  );

CREATE OR REPLACE TEMP TABLE sampled_seed_standard AS
SELECT * EXCLUDE (rn)
FROM standard_candidates
WHERE rn = 1;

CREATE OR REPLACE TEMP TABLE filler_standard AS
SELECT
  100000 + row_number() OVER (
    ORDER BY length(c.concept_name), c.concept_id
  ) AS seed_order,
  'filler:' || c.concept_name AS seed_name,
  c.concept_id,
  c.concept_name,
  c.domain_id,
  c.vocabulary_id,
  c.concept_class_id,
  c.standard_concept,
  c.concept_code
FROM concept c
WHERE c.domain_id = 'Drug'
  AND c.standard_concept = 'S'
  AND coalesce(c.invalid_reason, '') = ''
  AND c.vocabulary_id IN ('RxNorm', 'RxNorm Extension')
  AND c.concept_class_id IN (
    'Clinical Drug',
    'Clinical Drug Comp',
    'Branded Drug',
    'Branded Drug Comp',
    'Ingredient'
  )
  AND c.concept_id NOT IN (SELECT concept_id FROM sampled_seed_standard);

CREATE OR REPLACE TEMP TABLE sampled_standard AS
SELECT *
FROM (
  SELECT * FROM sampled_seed_standard
  UNION ALL
  SELECT * FROM filler_standard
)
ORDER BY seed_order
LIMIT 50;

COPY sampled_standard
TO 'fixtures/dev-drugs-50/standard_drug_concepts.csv' (HEADER, DELIMITER ',');

CREATE OR REPLACE TEMP TABLE mapped_non_standard AS
SELECT * FROM (
  SELECT
    ss.seed_order,
    ss.seed_name,
    ns.concept_id AS source_concept_id,
    ns.concept_name AS source_concept_name,
    ns.domain_id AS source_domain_id,
    ns.vocabulary_id AS source_vocabulary_id,
    ns.concept_class_id AS source_concept_class_id,
    ns.concept_code AS source_concept_code,
    ss.concept_id AS target_concept_id,
    ss.concept_name AS target_concept_name,
    ss.vocabulary_id AS target_vocabulary_id,
    ss.concept_class_id AS target_concept_class_id,
    row_number() OVER (
      PARTITION BY ss.concept_id
      ORDER BY length(ns.concept_name), ns.concept_id
    ) AS rn
  FROM sampled_standard ss
  JOIN concept_relationship cr
    ON cr.concept_id_2 = ss.concept_id
   AND cr.relationship_id = 'Maps to'
   AND coalesce(cr.invalid_reason, '') = ''
  JOIN concept ns
    ON ns.concept_id = cr.concept_id_1
  WHERE coalesce(ns.invalid_reason, '') = ''
    AND coalesce(ns.standard_concept, '') <> 'S'
) x
WHERE rn <= 5
ORDER BY seed_order, target_concept_id, rn;

COPY mapped_non_standard
TO 'fixtures/dev-drugs-50/non_standard_to_standard.csv' (HEADER, DELIMITER ',');

CREATE OR REPLACE TEMP TABLE source_terms AS
SELECT
  'STD_' || concept_id::VARCHAR AS source_code,
  concept_name AS source_name,
  1 AS source_frequency,
  concept_id AS expected_target_concept_id,
  concept_name AS expected_target_concept_name,
  'standard_seed' AS source_kind
FROM sampled_standard
UNION ALL
SELECT
  'NS_' || source_concept_id::VARCHAR AS source_code,
  source_concept_name AS source_name,
  1 AS source_frequency,
  target_concept_id AS expected_target_concept_id,
  target_concept_name AS expected_target_concept_name,
  'non_standard_maps_to_seed' AS source_kind
FROM mapped_non_standard;

COPY source_terms
TO 'fixtures/dev-drugs-50/source_terms.csv' (HEADER, DELIMITER ',');

COPY (
  SELECT DISTINCT
    expected_target_concept_id AS concept_id,
    expected_target_concept_name AS concept_name
  FROM source_terms
  ORDER BY concept_id
) TO 'fixtures/dev-drugs-50/expected_targets.csv' (HEADER, DELIMITER ',');

COPY (
  SELECT
    50 AS requested_seed_count,
    (SELECT count(*) FROM sampled_seed_standard) AS matched_seed_count,
    (SELECT count(*) FROM sampled_standard) AS sampled_standard_count,
    (SELECT count(*) FROM mapped_non_standard) AS non_standard_mapping_count,
    (SELECT count(*) FROM source_terms) AS source_term_count,
    list(seed_name ORDER BY seed_order)
      FILTER (
        WHERE seed_order NOT IN (
          SELECT seed_order FROM sampled_seed_standard
        )
      ) AS missing_seeds
  FROM seed_drugs
) TO 'fixtures/dev-drugs-50/metadata.json' (ARRAY true);
