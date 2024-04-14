use log::Log;
use log::{Level, LevelFilter};
use log::{Metadata, Record};

struct Logger;

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true // 允许全部级别的日志
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        use Level::*;
        let color = match record.level() {
            Error => 31,
            Warn => 93,
            Info => 34,
            Debug => 32,
            Trace => 90,
        };

        println!(
            "\u{1B}[{}m[{:<5}] [kernel] {}\u{1B}[0m",
            color,
            record.level(),
            record.args()
        );
    }

    fn flush(&self) {}
}

pub fn init() {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).unwrap();

    let level = option_env!("LOG")
        .and_then(|s: &'static str| s.parse().ok())
        .unwrap_or(LevelFilter::Off);
    log::set_max_level(level);
}
