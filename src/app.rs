use std::env;
use std::str::FromStr;

use clap::{crate_authors, crate_name, crate_version, App, Arg, ArgMatches};
use failure::ResultExt;
use log::info;
use rusoto_core::credential::ProfileProvider;
use rusoto_core::{HttpClient, Region};
use rusoto_logs::CloudWatchLogsClient;
use rusoto_sts::{StsAssumeRoleSessionCredentialsProvider, StsClient};

use crate::cmd;
use crate::errors;

const DEFAULT_REGION: &str = "ap-northeast-1";

struct GlobalOptions {
    profile: String,
    region: Region,
    role_arn: Option<String>,
    mfa_serial: Option<String>,
}

fn matches_string(matches: &ArgMatches<'static>, key: &str) -> Option<String> {
    matches.value_of(key).map(|s| s.to_string())
}

impl From<&ArgMatches<'static>> for GlobalOptions {
    fn from(matches: &ArgMatches<'static>) -> Self {
        GlobalOptions {
            profile: matches_string(matches, "PROFILE").unwrap(),
            region: Region::from_str(matches.value_of("REGION").unwrap_or(DEFAULT_REGION))
                .expect("Wrong region name"),
            role_arn: matches_string(matches, "ROLE_ARN"),
            mfa_serial: matches_string(matches, "MFA_SERIAL"),
        }
    }
}

fn cwlogs_client(
    region: Region,
    profile_name: &str,
    role_arn: Option<&str>,
    external_id: Option<&str>,
    mfa_serial: Option<&str>,
) -> CloudWatchLogsClient {
    env::set_var("AWS_PROFILE", profile_name);

    if let Some(role_arn) = role_arn {
        let sts = StsClient::new(region.clone());
        CloudWatchLogsClient::new_with(
            HttpClient::new().expect("Cannot create HttpClient for STS service"),
            StsAssumeRoleSessionCredentialsProvider::new(
                sts,
                role_arn.to_string(),
                "razy-awslogs".to_string(),
                None, // FIXME: コマンドラインオプションの指定を追加する
                None,
                None,
                mfa_serial.map(|s| s.to_string()),
            ),
            region,
        )
    } else {
        CloudWatchLogsClient::new_with(
            HttpClient::new().expect("Cannot create HttpClient for STS service"),
            ProfileProvider::new().expect("Cannot instantiate ProfileProvider"),
            region,
        )
    }
}

pub fn main() -> Result<(), errors::Error> {
    info!("create app");
    let mut app = app();

    info!("match agruments");
    let matches = app.clone().get_matches();

    let global_options = GlobalOptions::from(&matches);

    info!("create rusoto client");
    let client = cwlogs_client(
        global_options.region,
        global_options.profile.as_str(),
        global_options.role_arn.as_ref().map(|s| s.as_str()),
        None,
        global_options.mfa_serial.as_ref().map(|s| s.as_str()),
    );

    info!("invoke commands");
    match matches.subcommand() {
        ("get", Some(m)) => cmd::get::run(client, m),
        _ => {
            app.print_help().context(errors::ErrorKind::Clap)?;
            Err(errors::Error::from(errors::ErrorKind::NoSubCommand))
        }
    }
}

fn app() -> App<'static, 'static> {
    let app = App::new(crate_name!())
        .author(crate_authors!())
        .version(crate_version!())
        .about("")
        .arg(
            Arg::with_name("PROFILE")
                .help("AWS credentials profile")
                .short("p")
                .long("profile")
                .required(true)
                .takes_value(true)
                .value_name("PROFILE"),
        )
        .arg(
            Arg::with_name("REGION")
                .help("AWS region")
                .short("r")
                .long("region")
                .takes_value(true)
                .value_name("REGION"),
        )
        .arg(
            Arg::with_name("ROLE_ARN")
                .help("Role ARN")
                .long("role-arn")
                .takes_value(true)
                .value_name("ROLE_ARN"),
        )
        .arg(
            Arg::with_name("MFA_SERIAL")
                .help("The serial number of MFA")
                .long("mfa-serial")
                .takes_value(true)
                .value_name("MFA_SERIAL"),
        );

    // TODO: アプリの情報を設定する

    app.subcommand(cmd::get::sub_command("get"))
}
