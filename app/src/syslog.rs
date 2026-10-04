//! Minimal logger: syslog (`/dev/log`) on the device, stderr in a terminal.

use std::io::IsTerminal as _;
use std::os::unix::net::UnixDatagram;

use log::{Level, LevelFilter, Log, Metadata, Record};

struct Logger {
    socket: Option<UnixDatagram>,
}

pub fn init() {
    let socket = if std::io::stderr().is_terminal() {
        None
    } else {
        UnixDatagram::unbound()
            .ok()
            .filter(|s| s.connect("/dev/log").is_ok())
    };
    log::set_boxed_logger(Box::new(Logger { socket })).expect("logger already set");
    log::set_max_level(if std::env::var_os("MOBIX_DEBUG").is_some() {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
}

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if let Some(socket) = &self.socket {
            // facility user (1)
            let severity = match record.level() {
                Level::Error => 3,
                Level::Warn => 4,
                Level::Info => 6,
                Level::Debug | Level::Trace => 7,
            };
            let msg = format!(
                "<{}>mobix-app[{}]: {}",
                8 + severity,
                std::process::id(),
                record.args()
            );
            if socket.send(msg.as_bytes()).is_ok() {
                return;
            }
        }
        eprintln!("{}: {}", record.level(), record.args());
    }

    fn flush(&self) {}
}
