use mrq_core::config::MrqConfig;
use mrq_core::score::ScoringProfile;
use mrq_eval::metrics::recall_at_k;
use mrq_index::{
    snapshot::{create_snapshot, load_snapshot},
    types::{DocumentMetadata, ImageDocument, ImageSource, WriteBatch, WriteOperation},
    writer::IndexWriter,
};
use mrq_query::engine::{ImageQuery, ImageSourceQuery, QueryEngine, QueryMode, QueryOptions};

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn make_doc(id: u64, filename: &str) -> WriteOperation {
    let path = fixture_path(&format!("images/{}", filename));
    WriteOperation::Add(ImageDocument {
        image_id: id,
        external_id: filename.to_string(),
        source: ImageSource::Path(path.to_string_lossy().to_string()),
        metadata: DocumentMetadata {
            image_id: id,
            external_id: filename.to_string(),
            original_path: None,
            width: None,
            height: None,
            content_type: None,
            user_metadata_json: None,
        },
    })
}

fn build_test_index(db_path: &std::path::Path) -> IndexWriter {
    let cfg = MrqConfig::default();
    let mut writer = IndexWriter::create(db_path, cfg).unwrap();

    let ops: Vec<WriteOperation> = (1..=15)
        .map(|i| make_doc(i as u64, &format!("img{:03}.png", i)))
        .collect();

    let batch = WriteBatch {
        batch_id: "batch-001".to_string(),
        base_index_version: 0,
        operations: ops,
    };
    let result = writer.apply_batch(batch).unwrap();
    assert_eq!(result.added, 15);
    assert_eq!(result.new_version, 1);
    writer
}

#[test]
fn test_index_and_query_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("db");
    build_test_index(&db);

    let cfg = MrqConfig::default();
    let engine = QueryEngine::open(&db, cfg).unwrap();

    let query = ImageQuery {
        source: ImageSourceQuery::Path(
            fixture_path("queries/q001.png")
                .to_string_lossy()
                .to_string(),
        ),
        mode: QueryMode::Image,
    };
    let opts = QueryOptions {
        top_k: 5,
        candidate_limit: 50,
        scoring_profile: ScoringProfile::Image,
        include_explanation: true,
    };

    let resp = engine.query_by_image(query, opts).unwrap();
    assert_eq!(resp.index_version, 1);
    assert!(!resp.results.is_empty());
    assert!(resp.results.len() <= 5);

    // Top result should be img001 (exact copy of query)
    assert_eq!(resp.results[0].image_id, 1);

    // Scores descending
    for w in resp.results.windows(2) {
        assert!(w[0].score >= w[1].score);
    }

    // Explanation present
    assert!(resp.results[0].explanation.is_some());
}

#[test]
fn test_writebatch_idempotency() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("db");
    build_test_index(&db);

    let cfg = MrqConfig::default();
    let mut writer = IndexWriter::open(&db, cfg).unwrap();

    // Apply same batch again
    let ops = vec![make_doc(16, "img001.png")];
    let batch = WriteBatch {
        batch_id: "batch-001".to_string(), // Same ID as original
        base_index_version: 1,
        operations: ops,
    };
    let result = writer.apply_batch(batch).unwrap();

    // No-op: version unchanged
    assert_eq!(result.new_version, 1);
    assert_eq!(result.added, 0);
}

#[test]
fn test_remove_excluded_from_query() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("db");
    let mut writer = build_test_index(&db);

    // Remove image 1
    let remove_batch = WriteBatch {
        batch_id: "batch-remove".to_string(),
        base_index_version: 1,
        operations: vec![WriteOperation::Remove { image_id: 1 }],
    };
    let result = writer.apply_batch(remove_batch).unwrap();
    assert_eq!(result.removed, 1);
    assert_eq!(result.new_version, 2);

    let cfg = MrqConfig::default();
    let engine = QueryEngine::open(&db, cfg).unwrap();
    let query = ImageQuery {
        source: ImageSourceQuery::Path(
            fixture_path("queries/q001.png")
                .to_string_lossy()
                .to_string(),
        ),
        mode: QueryMode::Image,
    };
    let opts = QueryOptions::default();
    let resp = engine.query_by_image(query, opts).unwrap();

    let ids: Vec<u64> = resp.results.iter().map(|r| r.image_id).collect();
    assert!(
        !ids.contains(&1),
        "Deleted image 1 should not appear in results"
    );
}

#[test]
fn test_query_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("db");
    build_test_index(&db);

    let cfg = MrqConfig::default();
    let engine = QueryEngine::open(&db, cfg).unwrap();
    let query_path = fixture_path("queries/q001.png")
        .to_string_lossy()
        .to_string();

    let opts = QueryOptions {
        top_k: 10,
        candidate_limit: 50,
        scoring_profile: ScoringProfile::Image,
        include_explanation: false,
    };

    let r1 = engine
        .query_by_image(
            ImageQuery {
                source: ImageSourceQuery::Path(query_path.clone()),
                mode: QueryMode::Image,
            },
            opts.clone(),
        )
        .unwrap();
    let r2 = engine
        .query_by_image(
            ImageQuery {
                source: ImageSourceQuery::Path(query_path),
                mode: QueryMode::Image,
            },
            opts,
        )
        .unwrap();

    let ids1: Vec<u64> = r1.results.iter().map(|r| r.image_id).collect();
    let ids2: Vec<u64> = r2.results.iter().map(|r| r.image_id).collect();
    assert_eq!(ids1, ids2, "Query results must be deterministic");
}

#[test]
fn test_recall_at_k_inverted_vs_linear() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("db");
    build_test_index(&db);

    let cfg = MrqConfig::default();
    let engine = QueryEngine::open(&db, cfg).unwrap();
    let query_path = fixture_path("queries/q001.png")
        .to_string_lossy()
        .to_string();

    // Linear results (large candidate_limit ≈ full scan)
    let linear_opts = QueryOptions {
        top_k: 10,
        candidate_limit: 15, // All images
        scoring_profile: ScoringProfile::Image,
        include_explanation: false,
    };
    let linear_resp = engine
        .query_by_image(
            ImageQuery {
                source: ImageSourceQuery::Path(query_path.clone()),
                mode: QueryMode::Image,
            },
            linear_opts,
        )
        .unwrap();

    // Inverted index with tighter candidate limit
    let inv_opts = QueryOptions {
        top_k: 10,
        candidate_limit: 10,
        scoring_profile: ScoringProfile::Image,
        include_explanation: false,
    };
    let inv_resp = engine
        .query_by_image(
            ImageQuery {
                source: ImageSourceQuery::Path(query_path),
                mode: QueryMode::Image,
            },
            inv_opts,
        )
        .unwrap();

    let linear_ids: Vec<u64> = linear_resp.results.iter().map(|r| r.image_id).collect();
    let inv_ids: Vec<u64> = inv_resp.results.iter().map(|r| r.image_id).collect();

    let recall = recall_at_k(&linear_ids, &inv_ids, 5);
    assert!(
        recall >= 0.6,
        "Recall@5 inverted vs linear should be >= 0.6, got {:.2}",
        recall
    );
}

#[test]
fn test_snapshot_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("db");
    let snap = tmp.path().join("snap");
    let db2 = tmp.path().join("db2");

    build_test_index(&db);

    let manifest = create_snapshot(&db, &snap).unwrap();
    assert_eq!(manifest.index_version, 1);

    let loaded_version = load_snapshot(&snap, &db2).unwrap();
    assert_eq!(loaded_version, 1);

    // Query from restored index
    let cfg = MrqConfig::default();
    let engine = QueryEngine::open(&db2, cfg).unwrap();
    let resp = engine
        .query_by_image(
            ImageQuery {
                source: ImageSourceQuery::Path(
                    fixture_path("queries/q001.png")
                        .to_string_lossy()
                        .to_string(),
                ),
                mode: QueryMode::Image,
            },
            QueryOptions {
                top_k: 3,
                candidate_limit: 15,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!resp.results.is_empty());
}
