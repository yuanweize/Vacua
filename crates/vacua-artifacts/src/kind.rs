use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DeveloperArtifactKind {
    BuildOutput,
    DependencyTree,
    DependencyCache,
    CompilerCache,
    Index,
    GeneratedIntermediate,
    TestOutput,
    CoverageOutput,
    VirtualEnvironment,
    PackageDownloadCache,
    OtherGenerated,
}

impl DeveloperArtifactKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BuildOutput => "build_output",
            Self::DependencyTree => "dependency_tree",
            Self::DependencyCache => "dependency_cache",
            Self::CompilerCache => "compiler_cache",
            Self::Index => "index",
            Self::GeneratedIntermediate => "generated_intermediate",
            Self::TestOutput => "test_output",
            Self::CoverageOutput => "coverage_output",
            Self::VirtualEnvironment => "virtual_environment",
            Self::PackageDownloadCache => "package_download_cache",
            Self::OtherGenerated => "other_generated",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::BuildOutput => "Build Output",
            Self::DependencyTree => "Vendor / Dependency Tree",
            Self::DependencyCache => "Dependency Cache",
            Self::CompilerCache => "Compiler Cache",
            Self::Index => "IDE / Tool Index",
            Self::GeneratedIntermediate => "Generated Intermediate",
            Self::TestOutput => "Test Artifacts",
            Self::CoverageOutput => "Code Coverage Reports",
            Self::VirtualEnvironment => "Python Virtual Environment",
            Self::PackageDownloadCache => "Package Download Cache",
            Self::OtherGenerated => "Other Generated Development Data",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "build_output" => Some(Self::BuildOutput),
            "dependency_tree" => Some(Self::DependencyTree),
            "dependency_cache" => Some(Self::DependencyCache),
            "compiler_cache" => Some(Self::CompilerCache),
            "index" => Some(Self::Index),
            "generated_intermediate" => Some(Self::GeneratedIntermediate),
            "test_output" => Some(Self::TestOutput),
            "coverage_output" => Some(Self::CoverageOutput),
            "virtual_environment" => Some(Self::VirtualEnvironment),
            "package_download_cache" => Some(Self::PackageDownloadCache),
            "other_generated" => Some(Self::OtherGenerated),
            _ => None,
        }
    }

    pub fn is_dependency_state(&self) -> bool {
        matches!(
            self,
            Self::DependencyTree
                | Self::DependencyCache
                | Self::VirtualEnvironment
                | Self::PackageDownloadCache
        )
    }
}

impl std::fmt::Display for DeveloperArtifactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
