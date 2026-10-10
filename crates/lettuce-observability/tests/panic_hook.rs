#[test]
fn installed_hook_writes_a_real_panic_report() {
    let directory =
        std::env::temp_dir().join(format!("panic-hook-{}", lettuce_types::OperationId::new()));
    if let Some(path) = std::env::var_os("LETTUCE_PANIC_HOOK_TEST") {
        lettuce_observability::install_panic_reports(path.into());
        let _ = std::panic::catch_unwind(|| panic!("panic hook canary"));
        return;
    }
    std::fs::create_dir(&directory).expect("directory");
    let child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "installed_hook_writes_a_real_panic_report",
            "--nocapture",
        ])
        .env("LETTUCE_PANIC_HOOK_TEST", &directory)
        .output()
        .expect("child");
    assert!(child.status.success());
    let reports = lettuce_observability::LogDirectory::new(directory.clone());
    let names = reports.list().expect("reports");
    assert_eq!(names.len(), 1);
    let report = reports.read(&names[0]).expect("report");
    assert!(report.contains("payload: panic hook canary"));
    assert!(report.contains("location:"));
    assert!(report.contains("backtrace:"));
    std::fs::remove_dir_all(directory).expect("cleanup");
}
