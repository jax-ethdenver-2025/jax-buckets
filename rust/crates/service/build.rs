use std::env;
use std::process::Command;

fn report_build_profile() {
    println!(
        "cargo:rustc-env=BUILD_PROFILE={}",
        env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string())
    );
}

fn report_enabled_features() {
    let mut enabled_features: Vec<&str> = Vec::new();

    // TODO (amiller68): actually report those

    if enabled_features.is_empty() {
        enabled_features.push("none");
    }

    println!(
        "cargo:rustc-env=BUILD_FEATURES={}",
        enabled_features.join(",")
    );
}

fn report_repository_version() {
    let version = match env::var("CI_BUILD_REF") {
        Ok(val) if !val.is_empty() => val,
        _ => {
            match Command::new("git")
                .args(["describe", "--always", "--dirty", "--long", "--tags"])
                .output()
            {
                Ok(output) if output.status.success() => String::from_utf8(output.stdout)
                    .unwrap_or_else(|_| "unknown".to_string())
                    .trim()
                    .to_string(),
                _ => {
                    match Command::new("git")
                        .args(["rev-parse", "--short", "HEAD"])
                        .output()
                    {
                        Ok(output) if output.status.success() => String::from_utf8(output.stdout)
                            .unwrap_or_else(|_| "unknown".to_string())
                            .trim()
                            .to_string(),
                        _ => {
                            env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string())
                        }
                    }
                }
            }
        }
    };

    println!("cargo:rustc-env=REPO_VERSION={}", version);
}

fn report_build_timestamp() {
    let timestamp = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
}

fn report_rust_version() {
    let rust_version = match Command::new("rustc").args(["--version"]).output() {
        Ok(output) if output.status.success() => String::from_utf8(output.stdout)
            .unwrap_or_else(|_| "unknown".to_string())
            .trim()
            .to_string(),
        _ => "unknown".to_string(),
    };

    println!("cargo:rustc-env=RUST_VERSION={}", rust_version);
}

fn report_target_info() {
    if let Ok(target) = env::var("TARGET") {
        println!("cargo:rustc-env=BUILD_TARGET={}", target);
    }

    if let Ok(host) = env::var("HOST") {
        println!("cargo:rustc-env=BUILD_HOST={}", host);
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    report_build_profile();
    report_enabled_features();
    report_repository_version();
    report_build_timestamp();
    report_rust_version();
    report_target_info();
}
