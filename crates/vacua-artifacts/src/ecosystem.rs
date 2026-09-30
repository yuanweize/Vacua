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
pub enum DeveloperEcosystem {
    Xcode,
    SwiftPM,
    RustCargo,
    Node,
    Python,
    Gradle,
    Maven,
    CMake,
    Unknown,
}

impl DeveloperEcosystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Xcode => "xcode",
            Self::SwiftPM => "swift_pm",
            Self::RustCargo => "rust_cargo",
            Self::Node => "node",
            Self::Python => "python",
            Self::Gradle => "gradle",
            Self::Maven => "maven",
            Self::CMake => "cmake",
            Self::Unknown => "unknown",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Xcode => "Xcode",
            Self::SwiftPM => "Swift Package Manager",
            Self::RustCargo => "Rust Cargo",
            Self::Node => "Node.js",
            Self::Python => "Python",
            Self::Gradle => "Gradle",
            Self::Maven => "Apache Maven",
            Self::CMake => "CMake",
            Self::Unknown => "Unknown Ecosystem",
        }
    }

    pub fn default_rebuild_template(&self) -> &'static str {
        match self {
            Self::Xcode => "xcodebuild -scheme <Scheme> build",
            Self::SwiftPM => "swift build",
            Self::RustCargo => "cargo build",
            Self::Node => "npm run build # (or npm ci / pnpm install)",
            Self::Python => "python -m build # (or uv sync / pip install -r requirements.txt)",
            Self::Gradle => "./gradlew build",
            Self::Maven => "mvn compile",
            Self::CMake => "cmake --build .",
            Self::Unknown => "make # (or ecosystem build tool)",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "xcode" => Some(Self::Xcode),
            "swift_pm" | "swiftpm" | "swift" => Some(Self::SwiftPM),
            "rust_cargo" | "rust" | "cargo" => Some(Self::RustCargo),
            "node" | "nodejs" | "npm" | "yarn" | "pnpm" | "bun" => Some(Self::Node),
            "python" | "py" => Some(Self::Python),
            "gradle" => Some(Self::Gradle),
            "maven" | "mvn" => Some(Self::Maven),
            "cmake" => Some(Self::CMake),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

impl std::fmt::Display for DeveloperEcosystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
