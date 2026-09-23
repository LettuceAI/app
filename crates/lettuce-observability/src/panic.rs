use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static PANIC_REPORT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Chains a panic hook that writes each panic's report (time, thread,
/// location, payload and a forced backtrace) to its own file in `directory`
/// and logs where it went, then runs the previous hook.
pub fn install_panic_reports(directory: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic payload".to_owned());
        let location = info.location().map_or_else(
            || "unknown".to_owned(),
            |location| format!("{}:{}", location.file(), location.line()),
        );
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unnamed").to_owned();
        let report = format!(
            "timestamp: {}\nthread: {thread_name}\nlocation: {location}\npayload: {payload}\n\npanic_info: {info}\n\nbacktrace:\n{}\n",
            chrono::Local::now().to_rfc3339(),
            std::backtrace::Backtrace::force_capture()
        );
        eprintln!("panic report:\n{report}");
        match write_panic_report(&directory, &thread_name, &report) {
            Ok(path) => tracing::error!(
                %location,
                thread = %thread_name,
                report = %path.display(),
                %payload,
                "rust panic captured"
            ),
            Err(error) => tracing::error!(
                %location,
                thread = %thread_name,
                %error,
                %payload,
                "rust panic captured; the report could not be written"
            ),
        }
        previous(info);
    }));
}

fn write_panic_report(
    directory: &Path,
    thread_name: &str,
    report: &str,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(format!(
        "panic-{}-p{}-{}-{}.log",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S%.3f"),
        std::process::id(),
        report_name_segment(thread_name),
        PANIC_REPORT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, report)?;
    Ok(path)
}

fn report_name_segment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "unnamed".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_get_their_own_sanitized_file() {
        let directory = std::env::temp_dir().join(format!(
            "panic-reports-{}",
            lettuce_types::OperationId::new()
        ));
        let first = write_panic_report(&directory, "tokio worker #1", "first").expect("first");
        let second = write_panic_report(&directory, "", "second").expect("second");
        assert_ne!(first, second);
        let name = first
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name");
        assert!(name.starts_with("panic-") && name.contains("-tokio_worker__1-"));
        assert!(
            second
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("-unnamed-"))
        );
        assert_eq!(fs::read_to_string(&first).expect("read"), "first");
        let _ = fs::remove_dir_all(directory);
    }
}
