//! The version the app reports: the package version with the llama.cpp GPU
//! backend it was built for, so updates can pick the matching build.

/// `package_version` followed by `-cuda`, `-rocm` or `-vulkan` when this
/// build's local model runtime was compiled for that backend.
#[must_use]
pub fn app_version(package_version: &str) -> String {
    let mut version = package_version.to_owned();
    if let Some(backend) = gpu_suffix() {
        version.push('-');
        version.push_str(backend);
    }
    version
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn gpu_suffix() -> Option<&'static str> {
    lettuce_local_llm::engine::compiled_gpu_backends()
        .into_iter()
        .find(|backend| matches!(*backend, "cuda" | "rocm" | "vulkan"))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
const fn gpu_suffix() -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cpu_build_reports_the_package_version() {
        let expected = match gpu_suffix() {
            Some(backend) => format!("2.3.0-{backend}"),
            None => "2.3.0".to_owned(),
        };
        assert_eq!(app_version("2.3.0"), expected);
    }
}
