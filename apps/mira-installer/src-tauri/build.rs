/// Runs the main step for the installer build script.
fn main() {
    configure_build_environment();
    #[cfg(target_os = "windows")]
    {
        let windows = tauri_build::WindowsAttributes::new().app_manifest(
            r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#,
        );
        let attributes = tauri_build::Attributes::new().windows_attributes(windows);
        tauri_build::try_build(attributes).expect("failed to run Tauri build script");
    }

    #[cfg(not(target_os = "windows"))]
    tauri_build::build()
}

fn configure_build_environment() {
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
