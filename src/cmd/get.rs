use std::sync::mpsc::channel;

use chrono::prelude::*;
use clap::{App, Arg, ArgMatches, SubCommand};
use failure::{format_err, ResultExt};
use futures::prelude::*;
use lazy_static::lazy_static;
use log::{debug, info};
use rusoto_logs::CloudWatchLogsClient;

use crate::errors;

mod event;
mod printer;
mod stream;

pub struct GetOptions<'a> {
    group_name: &'a str,
    stream_name: Option<&'a str>,
    filter_expression: Option<&'a str>,
    start_time: Option<DateTime<Utc>>, // TODO: DateTime化
    end_time: Option<DateTime<Utc>>,   // TODO: DateTime化
    watch: bool,
    use_prefix: bool,
}

lazy_static! {
    static ref TZ_ASIA_TOKYO: FixedOffset = FixedOffset::east(9 * 60 * 60);
}

fn from_jst_text(jst_text: &str) -> DateTime<Utc> {
    let dt = TZ_ASIA_TOKYO
        .datetime_from_str(jst_text, "%Y-%m-%d %H:%M:%S")
        .expect("failed to parse as JST time");

    dt.with_timezone(&Utc)
}

impl<'a> From<&'a ArgMatches<'a>> for GetOptions<'a> {
    fn from(matches: &'a ArgMatches<'a>) -> Self {
        GetOptions {
            group_name: matches.value_of("GROUP_NAME").unwrap(),
            filter_expression: matches.value_of("FILTER_EXPRESSION"),
            start_time: matches.value_of("START_TIME").map(from_jst_text),
            end_time: matches.value_of("END_TIME").map(from_jst_text),
            stream_name: matches.value_of("STREAM_NAME"),
            watch: matches.is_present("WATCH"),
            use_prefix: !matches.is_present("NO_PREFIX"),
        }
    }
}

pub fn sub_command(s: &'static str) -> App<'static, 'static> {
    SubCommand::with_name(s)
        .about("TODO: write about the command")
        .arg(
            Arg::with_name("GROUP_NAME")
                .help("The name of the log group")
                .short("g")
                .long("group")
                .required(true)
                .takes_value(true)
                .value_name("GROUP_NAME"),
        )
        .arg(
            Arg::with_name("FILTER_EXPRESSION")
                .help("The filter pattern to use. If not provided, all the events are matched.")
                .short("f")
                .long("filter-pattern")
                .takes_value(true)
                .value_name("FILTER_EXPRESSION"),
        )
        .arg(
            Arg::with_name("START_TIME")
                .help("The start of the time range")
                .long("start-time")
                .takes_value(true)
                .value_name("TIME"),
        )
        .arg(
            Arg::with_name("END_TIME")
                .help("The end of the time range")
                .long("end-time")
                .takes_value(true)
                .value_name("TIME"),
        )
        .arg(
            Arg::with_name("STREAM_NAME")
                .help("The name of log stream")
                .short("s")
                .long("stream")
                .takes_value(true)
                .value_name("STREAM_NAME"),
        )
        .arg(
            Arg::with_name("WATCH")
                .help("Do not stop when end of log is reached.")
                .short("w")
                .long("watch"),
        ).arg(
        Arg::with_name("NO_PREFIX")
            .help("Do not display the time and stream name in the event at the begin of the line.")
            .long("no-prefix"),
    )
}

type LogEventStream = Stream<Item = stream::LogEventsReadResponse, Error = errors::Error> + Send;

trait Runner {
    fn run(
        &self,
        log_events: Box<LogEventStream>,
        printer: Box<printer::Printer>,
    ) -> Box<Future<Item = (), Error = errors::Error> + Send>;
}

struct OneShotRunner {}

impl Default for OneShotRunner {
    fn default() -> Self {
        OneShotRunner {}
    }
}

impl Runner for OneShotRunner {
    fn run(
        &self,
        log_events: Box<LogEventStream>,
        printer: Box<printer::Printer>,
    ) -> Box<Future<Item = (), Error = errors::Error> + Send> {
        info!("iterate log events stream");

        let fut = log_events.for_each(move |res| {
            printer.print_events(&res.events);
            Ok(())
        });

        Box::new(fut)
    }
}

#[derive(Debug)]
enum Payload {
    Done,
    Failure(errors::Error),
}

fn create_log_events_stream(
    client: CloudWatchLogsClient,
    options: &GetOptions,
) -> Result<Box<LogEventStream>, errors::Error> {
    let request = stream::LogEventsReadRequest {
        start_time: options.start_time,
        end_time: options.end_time,
    };
    Ok(match options.filter_expression {
        Some(filter) => {
            stream::filter_log_events_stream(
                client,
                options.group_name.to_string(),
                None, // NOTE: ストリームの指定どうするか確認する
                filter.to_string(),
                request,
            )
        }
        None => {
            // get-log-eventsの場合はストリーム名必須
            let stream_name = options.stream_name.map(|s| Ok(s)).unwrap_or(
                Err(format_err!(
                    "Need to specify '--stream' when omit '--filter-expression'"
                ))
                .context(errors::ErrorKind::InsufficientArguments),
            )?;

            stream::get_log_events_stream(
                client,
                options.group_name.to_string(),
                stream_name.to_string(),
                request,
            )
        }
    })
}

fn create_printer(options: &GetOptions) -> Box<printer::Printer> {
    if options.use_prefix {
        Box::new(printer::LogPrinter::default()) as Box<printer::Printer>
    } else {
        Box::new(printer::MessagePrinter::default()) as Box<printer::Printer>
    }
}

pub fn run(client: CloudWatchLogsClient, matches: &ArgMatches) -> Result<(), errors::Error> {
    info!("parse get options");
    let options = GetOptions::from(matches);

    // ログの読み取り方法を決める (get-log-events or filter-log-events)
    info!("create reader");
    let stream = create_log_events_stream(client, &options)?;

    // ログの表示方法を決める
    info!("create printer");
    let printer = create_printer(&options);

    // 実行方法を決める
    // TODO: Watch用のランナーを作る
    info!("create runner");
    let runner = OneShotRunner::default();

    let (sender, receiver) = channel();
    let sender_ok = sender.clone();
    let sender_err = sender.clone();

    info!("create futures to run");
    let f = runner
        .run(stream, printer)
        .map_err(move |e| {
            sender_ok.send(Payload::Failure(e)).unwrap();
            ()
        })
        .map(move |_| {
            sender_err.clone().send(Payload::Done).unwrap();
            ()
        });

    info!("run!!");
    tokio::run(f);

    info!("receive result");
    match receiver.recv().context(errors::ErrorKind::SyncChannel)? {
        Payload::Done => Ok(()),
        Payload::Failure(e) => {
            debug!("error occurred: {}", e);
            Err(e)
        }
    }
}
