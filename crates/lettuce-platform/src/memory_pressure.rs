use std::{fmt, io, sync::Arc};

type Callback = Arc<dyn Fn() + Send + Sync>;

pub struct MemoryPressureMonitor {
    _inner: native::Monitor,
}

impl fmt::Debug for MemoryPressureMonitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryPressureMonitor").finish_non_exhaustive()
    }
}

impl MemoryPressureMonitor {
    pub fn subscribe(callback: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        Ok(Self { _inner: native::Monitor::new(Arc::new(callback))? })
    }
}

#[cfg(target_os = "linux")]
mod native {
    use std::{fs::OpenOptions, io::{self, Write}, sync::Arc, thread::JoinHandle};
    use rustix::{event::{EventfdFlags, PollFd, PollFlags, eventfd, poll}, fd::OwnedFd};
    use super::Callback;

    const PSI_TRIGGER: &[u8] = b"some 200000 2000000\0";

    pub(super) struct Monitor {
        stop: Arc<OwnedFd>,
        thread: Option<JoinHandle<()>>,
    }

    impl Monitor {
        pub(super) fn new(callback: Callback) -> io::Result<Self> {
            let requested = std::env::var_os("MEMORY_PRESSURE_WATCH").map(std::path::PathBuf::from);
            let path = requested.filter(|path| path.starts_with("/sys/fs/cgroup") && path.file_name().is_some_and(|name| name == "memory.pressure"))
                .unwrap_or_else(|| "/proc/pressure/memory".into());
            let mut pressure = OpenOptions::new().read(true).write(true).open(path)?;
            pressure.write_all(PSI_TRIGGER)?;
            let stop = Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?);
            let stopped = stop.clone();
            let thread = std::thread::Builder::new().name("memory-pressure".into()).spawn(move || {
                let mut descriptors = [
                    PollFd::new(stopped.as_ref(), PollFlags::IN),
                    PollFd::new(&pressure, PollFlags::PRI),
                ];
                loop {
                    match poll(&mut descriptors, None) {
                        Err(rustix::io::Errno::INTR) => continue,
                        Err(_) => break,
                        Ok(_) => {}
                    }
                    if !descriptors[0].revents().is_empty() { break; }
                    if descriptors[1].revents().contains(PollFlags::PRI) {
                        callback();
                    } else if !descriptors[1].revents().is_empty() { break; }
                }
            })?;
            Ok(Self { stop, thread: Some(thread) })
        }
    }

    impl Drop for Monitor {
        fn drop(&mut self) {
            let signal = 1_u64.to_ne_bytes();
            while rustix::io::write(self.stop.as_ref(), &signal) == Err(rustix::io::Errno::INTR) {}
            if let Some(thread) = self.thread.take() { let _ = thread.join(); }
        }
    }
}

#[cfg(target_os = "windows")]
mod native {
    use std::{ffi::c_void, io, sync::Arc, thread::JoinHandle};
    use super::Callback;

    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::{Memory::{CreateMemoryResourceNotification, HighMemoryResourceNotification, LowMemoryResourceNotification},
            Threading::{CreateEventW, SetEvent, WaitForMultipleObjects, INFINITE}},
    };

    struct Signals { stop: usize, low: usize, high: usize }
    impl Drop for Signals {
        fn drop(&mut self) {
            for handle in [self.stop, self.low, self.high].into_iter().filter(|handle| *handle != 0) {
                unsafe { CloseHandle(handle as *mut c_void); }
            }
        }
    }
    pub(super) struct Monitor { signals: Arc<Signals>, thread: Option<JoinHandle<()>> }
    impl Monitor {
        pub(super) fn new(callback: Callback) -> io::Result<Self> {
            let signals = Arc::new(Signals {
                stop: unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) } as usize,
                low: unsafe { CreateMemoryResourceNotification(LowMemoryResourceNotification) } as usize,
                high: unsafe { CreateMemoryResourceNotification(HighMemoryResourceNotification) } as usize,
            });
            if [signals.stop, signals.low, signals.high].contains(&0) { return Err(io::Error::last_os_error()); }
            let owned = signals.clone();
            let thread = std::thread::Builder::new().name("memory-pressure".into()).spawn(move || {
                let mut low = true;
                loop {
                    let handles = [owned.stop as *mut c_void, (if low { owned.low } else { owned.high }) as *mut c_void];
                    match unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) } {
                        WAIT_OBJECT_0 => break,
                        value if value == WAIT_OBJECT_0 + 1 => { if low { callback(); } low = !low; }
                        _ => break,
                    }
                }
            })?;
            Ok(Self { signals, thread: Some(thread) })
        }
    }
    impl Drop for Monitor {
        fn drop(&mut self) {
            unsafe { SetEvent(self.signals.stop as *mut c_void); }
            if let Some(thread) = self.thread.take() { let _ = thread.join(); }
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod native {
    use std::io;
    use block2::RcBlock;
    use dispatch2::{DispatchObject, DispatchQueue, DispatchRetained, DispatchSource,
        _dispatch_source_type_memorypressure, dispatch_source_memorypressure_flags_t as Pressure};
    use super::Callback;

    pub(super) struct Monitor {
        source: DispatchRetained<DispatchSource>,
        _queue: DispatchRetained<DispatchQueue>,
    }
    impl Monitor {
        pub(super) fn new(callback: Callback) -> io::Result<Self> {
            let queue = DispatchQueue::new("lettuce-memory-pressure", None);
            let flags = Pressure::DISPATCH_MEMORYPRESSURE_WARN.0 | Pressure::DISPATCH_MEMORYPRESSURE_CRITICAL.0;
            let source = unsafe { DispatchSource::new(
                std::ptr::addr_of!(_dispatch_source_type_memorypressure).cast_mut(),
                0, flags as usize, Some(&queue),
            ) };
            let handler = RcBlock::new(move || {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()));
            });
            unsafe { source.set_event_handler_with_block(RcBlock::as_ptr(&handler)); }
            source.activate();
            Ok(Self { source, _queue: queue })
        }
    }
    impl Drop for Monitor {
        fn drop(&mut self) { self.source.cancel(); }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos", target_os = "ios")))]
mod native {
    use std::io;
    use super::Callback;
    pub(super) struct Monitor;
    impl Monitor {
        pub(super) fn new(_callback: Callback) -> io::Result<Self> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "the host delivers native memory warnings"))
        }
    }
}
