use usagi_contracts::catalog::Provenance;
use usagi_contracts::mapper::MapperDrugBatchResponse;
use usagi_contracts::search::SearchBatchResponse;

#[test]
fn search_batch_response_serializes_provenance() {
    let response = SearchBatchResponse {
        mode: "lexical_tantivy".to_string(),
        items: Vec::new(),
        provenance: Provenance {
            catalog_artifact_id: Some("catalog-v1".to_string()),
            model_artifact_id: None,
            index_artifact_id: Some("tantivy-v1".to_string()),
        },
    };

    let value = serde_json::to_value(response).expect("response json");

    assert_eq!(value["provenance"]["catalog_artifact_id"], "catalog-v1");
    assert_eq!(value["provenance"]["index_artifact_id"], "tantivy-v1");
}

#[test]
fn mapper_batch_response_serializes_provenance() {
    let response = MapperDrugBatchResponse {
        mode: "thirawat_tachiom".to_string(),
        items: Vec::new(),
        provenance: Provenance {
            catalog_artifact_id: Some("catalog-v1".to_string()),
            model_artifact_id: Some("sidataplus/THIRAWAT-SapBERT".to_string()),
            index_artifact_id: Some("tachiom-v1".to_string()),
        },
    };

    let value = serde_json::to_value(response).expect("response json");

    assert_eq!(
        value["provenance"]["model_artifact_id"],
        "sidataplus/THIRAWAT-SapBERT"
    );
    assert_eq!(value["provenance"]["index_artifact_id"], "tachiom-v1");
}
