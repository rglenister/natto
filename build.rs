use std::process::Command;

fn main() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let git_status = Command::new("git").args(["diff-index", "--quiet", "HEAD", "--"]).status();

    let dirty = match git_status {
        Ok(status) if status.success() => "",
        _ => "-dirty",
    };

    println!("cargo:rustc-env=GIT_HASH={}{}", git_hash, dirty);

    println!("cargo:rustc-env=BUILD_DATE={}", chrono::Utc::now());
}
