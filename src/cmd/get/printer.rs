use ansi_term::Color;

use super::TZ_ASIA_TOKYO;
use crate::cmd::get::event::LogEvent;

pub trait Printer: Send {
    fn print_events(&self, events: &Vec<LogEvent>);

    fn puts(&self, text: &str) {
        if text.ends_with("\n") {
            print!("{}", text);
        } else {
            println!("{}", text);
        }
    }
}

#[derive(Clone)]
pub struct MessagePrinter {}

unsafe impl Send for MessagePrinter {}

impl Default for MessagePrinter {
    fn default() -> Self {
        MessagePrinter {}
    }
}

impl Printer for MessagePrinter {
    fn print_events(&self, events: &Vec<LogEvent>) {
        for event in events.iter() {
            self.puts(format!("{}", event.message).as_str());
        }
    }
}

#[derive(Clone)]
pub struct LogPrinter {
    enable_color: bool,
}

unsafe impl Send for LogPrinter {}

impl LogPrinter {
    fn decorate(&self, text: String) -> String {
        if self.enable_color {
            Color::Green.paint(&text).to_string()
        } else {
            text
        }
    }
}

impl Default for LogPrinter {
    fn default() -> Self {
        LogPrinter {
            enable_color: atty::is(atty::Stream::Stdout),
        }
    }
}

impl Printer for LogPrinter {
    fn print_events(&self, events: &Vec<LogEvent>) {
        let tz = TZ_ASIA_TOKYO.clone();
        for event in events.iter() {
            let prefix = self.decorate(format!(
                "[{}]",
                // event.timestamp.with_timezone(&tz).to_rfc3339()
                event
                    .timestamp
                    .with_timezone(&tz)
                    .format("%Y-%m-%d %H:%M:%S"),
            ));

            self.puts(format!("{} {}", prefix, event.message).as_str());
        }
    }
}
