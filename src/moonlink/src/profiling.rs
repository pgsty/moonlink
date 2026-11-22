use std::fmt::Write as _;
use std::io;

pub fn write_flamegraph(report: &pprof::Report, output: impl io::Write) -> io::Result<()> {
    let mut stacks = Vec::with_capacity(report.data.len());
    for (sample, count) in &report.data {
        let mut stack = sample.thread_name_or_id();
        for frame in sample.frames.iter().rev() {
            for symbol in frame.iter().rev() {
                write!(stack, ";{symbol}").unwrap();
            }
        }
        write!(stack, " {count}").unwrap();
        stacks.push(stack);
    }
    if stacks.is_empty() {
        return Ok(());
    }
    inferno::flamegraph::from_lines(
        &mut Default::default(),
        stacks.iter().map(String::as_str),
        output,
    )
}
