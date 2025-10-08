// build.rs

fn report_build_profile() {
    println!(
        "cargo:rustc-env=BUILD_PROFILE={}",
        std::env::var("PROFILE").unwrap()
    );
}

fn report_enabled_features() {
    let mut enabled_features: Vec<&str> = Vec::new();

    if enabled_features.is_empty() {
        enabled_features.push("none");
    }

    println!(
        "cargo:rustc-env=BUILD_FEATURES={}",
        enabled_features.join(",")
    );
}

fn report_repository_version() {
    let version = match std::env::var("CI_BUILD_REF") {
        Ok(val) if !val.is_empty() => val,
        _ => {
            let git_describe = std::process::Command::new("git")
                .args(["describe", "--always", "--dirty", "--long", "--tags"])
                .output()
                .unwrap();

            String::from_utf8(git_describe.stdout).unwrap()
        }
    };

    println!("cargo:rustc-env=REPO_VERSION={}", version);
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let _target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    report_build_profile();
    report_enabled_features();
    report_repository_version();
}
