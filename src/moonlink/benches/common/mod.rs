use criterion::profiler::Profiler;
use pprof::ProfilerGuard;
use std::{fs, path::Path};

#[derive(Default)]
pub struct FlamegraphProfiler(Option<ProfilerGuard<'static>>);

impl Profiler for FlamegraphProfiler {
    fn start_profiling(&mut self, _: &str, _: &Path) {
        self.0 = Some(ProfilerGuard::new(100).unwrap());
    }

    fn stop_profiling(&mut self, _: &str, directory: &Path) {
        if let Some(guard) = self.0.take() {
            let report = guard.report().build().unwrap();
            fs::create_dir_all(directory).unwrap();
            let output = fs::File::create(directory.join("flamegraph.svg")).unwrap();
            moonlink::profiling::write_flamegraph(&report, output).unwrap();
        }
    }
}
