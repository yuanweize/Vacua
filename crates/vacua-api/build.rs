use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=VACUA_GIT_SHA");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");

    let git_sha = if let Ok(sha) = std::env::var("VACUA_GIT_SHA") {
        sha.trim().to_string()
    } else {
        // Fall back to git rev-parse HEAD if git is available
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string())
    };

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=VACUA_GIT_SHA={}", git_sha);
    println!("cargo:rustc-env=VACUA_TARGET={}", target);
    println!("cargo:rustc-env=VACUA_PROFILE={}", profile);
}
