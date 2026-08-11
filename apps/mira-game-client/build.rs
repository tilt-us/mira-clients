/// Validates and embeds the deployment environment for this game-client build.
fn main() {
    println!("cargo:rerun-if-env-changed=MIRA_ENV");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let environment = match std::env::var("MIRA_ENV") {
        Ok(value) if matches!(value.as_str(), "dev" | "staging" | "prod") => value,
        Ok(value) => panic!("Invalid MIRA_ENV={value:?}. Use one of: dev, staging, prod."),
        Err(_) if profile == "release" => {
            panic!("MIRA_ENV is required for release builds. Use one of: dev, staging, prod.")
        }
        Err(_) => "dev".to_string(),
    };

    println!("cargo:rustc-env=MIRA_ENV={environment}");
}
