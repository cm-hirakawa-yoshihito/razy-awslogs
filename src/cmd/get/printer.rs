use ansi_term::Color;
use chrono::prelude::*;

use super::TZ_ASIA_TOKYO;
use crate::cmd::get::event::LogEvent;

pub trait Printer: Send {
    fn print_events(&self, events: &Vec<LogEvent>);
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
        // TODO: 出力先を可変にするか検討すること
        for event in events.iter() {
            if event.message.ends_with("\n") {
                print!("{}", event.message);
            } else {
                println!("{}", event.message);
            }
        }
    }
}

#[derive(Clone)]
pub struct LogPrinter {}

unsafe impl Send for LogPrinter {}

impl Default for LogPrinter {
    fn default() -> Self {
        LogPrinter {}
    }
}

impl Printer for LogPrinter {
    fn print_events(&self, events: &Vec<LogEvent>) {
        let tz = TZ_ASIA_TOKYO.clone();
        for event in events.iter() {
            let prefix = Color::Green.paint(format!(
                "[{}]",
                // event.timestamp.with_timezone(&tz).to_rfc3339()
                event
                    .timestamp
                    .with_timezone(&tz)
                    .format("%Y-%m-%d %H:%M:%S")
            ));

            if event.message.ends_with("\n") {
                print!("{} {}", prefix, event.message);
            } else {
                println!("{} {}", prefix, event.message);
            }
        }
    }
}
