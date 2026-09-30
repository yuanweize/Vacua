use schemars::schema_for;
use vacua_api::{get_build_info, ArtifactChecksumV1, ReleaseManifestV1};

#[test]
fn test_build_info_version_matches_package() {
    let info = get_build_info();
    assert_eq!(info.schema_version, "vacua.build-info.v1");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert!(!info.target.is_empty());
    assert!(!info.profile.is_empty());
}

#[test]
fn test_build_info_git_sha_nonempty() {
    let info = get_build_info();
    assert!(!info.git_commit.is_empty());
    // Commit should not be whitespace
    assert_eq!(info.git_commit.trim(), info.git_commit);
}

#[test]
fn test_release_manifest_schema_valid() {
    let schema = schema_for!(ReleaseManifestV1);
    let schema_json = serde_json::to_value(&schema).unwrap();
    assert!(schema_json.is_object());
    assert!(schema_json.get("title").is_some() || schema_json.get("$schema").is_some());
}

#[test]
fn test_release_manifest_roundtrip_and_digests() {
    let manifest = ReleaseManifestV1 {
        schema_version: "vacua.release-manifest.v1".to_string(),
        version: "0.6.1".to_string(),
        tag: "v0.6.1".to_string(),
        git_commit: "2b62283193cef3e67e31ccd4df99da3f2723c279".to_string(),
        repository: "yuanweize/vacua".to_string(),
        target: "aarch64-apple-darwin".to_string(),
        artifacts: vec![
            ArtifactChecksumV1 {
                name: "vacua-v0.6.1-aarch64-apple-darwin.tar.gz".to_string(),
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
            },
            ArtifactChecksumV1 {
                name: "Vacua-v0.6.1-macos-arm64-unsigned.zip".to_string(),
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
            },
        ],
        rustc_version: Some("rustc 1.85.0".to_string()),
        swift_version: Some("swift-driver version: 1.115".to_string()),
        xcode_version: Some("Xcode 16.2".to_string()),
        github_run_id: Some("12345678".to_string()),
    };

    let serialized = serde_json::to_string_pretty(&manifest).unwrap();
    let deserialized: ReleaseManifestV1 = serde_json::from_str(&serialized).unwrap();
    assert_eq!(manifest, deserialized);
    assert_eq!(deserialized.artifacts.len(), 2);
    assert_eq!(
        deserialized.artifacts[0].name,
        "vacua-v0.6.1-aarch64-apple-darwin.tar.gz"
    );
}
