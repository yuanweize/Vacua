use std::fs;
use std::path::PathBuf;
use vacua_api::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("api");

    println!(
        "Verifying and formatting contract fixtures in: {:?}",
        fixtures_dir
    );

    roundtrip::<ServerCapabilitiesV1>(&fixtures_dir.join("capabilities-v1.json"))?;
    roundtrip::<StorageSummaryV1>(&fixtures_dir.join("storage-summary-v1.json"))?;
    roundtrip::<CandidateListResponseV1>(&fixtures_dir.join("candidate-list-v1.json"))?;
    roundtrip::<CandidateDetailV1>(&fixtures_dir.join("candidate-detail-v1.json"))?;
    roundtrip::<DuplicateListResponseV1>(&fixtures_dir.join("duplicate-list-v1.json"))?;
    roundtrip::<DuplicateGroupDetailV1>(&fixtures_dir.join("duplicate-detail-v1.json"))?;
    roundtrip::<ApplicationListResponseV1>(&fixtures_dir.join("application-list-v1.json"))?;
    roundtrip::<ApplicationDetailV1>(&fixtures_dir.join("application-detail-v1.json"))?;
    roundtrip::<SnapshotListResponseV1>(&fixtures_dir.join("snapshot-list-v1.json"))?;
    roundtrip::<SnapshotDiffV1>(&fixtures_dir.join("snapshot-diff-v1.json"))?;
    roundtrip::<CleanupSimulationV1>(&fixtures_dir.join("cleanup-simulation-v1.json"))?;
    roundtrip::<CleanupPlanProposalV1>(&fixtures_dir.join("cleanup-proposal-v1.json"))?;

    println!("All 12 contract fixtures validated and proven through real Rust DTO serialization!");
    Ok(())
}

fn roundtrip<T: serde::de::DeserializeOwned + serde::Serialize>(
    path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let parsed: T = serde_json::from_str(&content)?;
    let re_serialized = serde_json::to_string_pretty(&parsed)?;
    fs::write(path, format!("{}\n", re_serialized))?;
    Ok(())
}
