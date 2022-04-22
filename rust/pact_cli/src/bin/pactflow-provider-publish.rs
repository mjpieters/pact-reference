//! CLI to publish provider contracts to a Pact broker.

#![warn(missing_docs)]

use std::env;
use std::fs::File;

use anyhow::{anyhow, Context};
use clap::{App, AppSettings, Arg, ArgMatches, ErrorKind};
use log::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml::Value as YamlValue;

use pact_cli::{glob_value, setup_loggers};
use pact_models::http_utils;
use pact_models::http_utils::HttpAuth;

fn setup_app<'a, 'b>(program: &str, version: &'b str) -> App<'a, 'b> {
  App::new(program)
    .version(version)
    .about("Pactflow provider publisher")
    .arg(
      Arg::with_name("loglevel")
        .short("l")
        .long("loglevel")
        .takes_value(true)
        .use_delimiter(false)
        .possible_values(&["error", "warn", "info", "debug", "trace", "none"])
        .help("Log level (defaults to warn)"),
    )
    .arg(
      Arg::with_name("contentFile")
        .short("f")
        .long("contentFile")
        .required(true)
        .takes_value(true)
        .use_delimiter(false)
        .multiple(false)
        .number_of_values(1)
        .empty_values(false)
        .help("Provider specification to publish"),
    )
    .arg(
      Arg::with_name("token")
        .short("t")
        .long("token")
        .required(true)
        .takes_value(true)
        .use_delimiter(false)
        .number_of_values(1)
        .empty_values(false)
        .help("Bearer token to use to publish with"),
    )
    .arg(
      Arg::with_name("baseURL")
        .short("b")
        .long("baseURL")
        .required(true)
        .takes_value(true)
        .use_delimiter(false)
        .number_of_values(1)
        .empty_values(false)
        .help("The base URL of your Pactflow account"),
    )
    .arg(
      Arg::with_name("application")
        .short("a")
        .long("application")
        .required(true)
        .takes_value(true)
        .use_delimiter(false)
        .number_of_values(1)
        .empty_values(false)
        .help("The name of the provider API application"),
    )
    .arg(
      Arg::with_name("version")
        .short("V")
        .long("version")
        .takes_value(true)
        .use_delimiter(false)
        .number_of_values(1)
        .empty_values(false)
        .help("The version of the provider API application"),
    )
    .arg(
      Arg::with_name("contractType")
        .short("c")
        .long("contractType")
        .takes_value(true)
        .use_delimiter(false)
        .number_of_values(1)
        .empty_values(false)
        .help("The type of provider contract contract - currently supported oas only"),
    )
    .arg(
      Arg::with_name("verificationResult")
        .short("r")
        .long("verificationResult")
        .required(true)
        .takes_value(true)
        .use_delimiter(false)
        .number_of_values(1)
        .empty_values(false)
        .help("A boolean value indicating if the tests passed or failed (one of true or false)"),
    )
    .arg(
      Arg::with_name("resultsFile")
        .short("F")
        .long("resultsFile")
        .takes_value(true)
        .use_delimiter(false)
        .multiple(false)
        .number_of_values(1)
        .empty_values(false)
        .help("Provider verfication result result file to publish"),
    )
    .arg(
      Arg::with_name("tool")
        .short("T")
        .long("tool")
        .takes_value(true)
        .use_delimiter(false)
        .number_of_values(1)
        .empty_values(false)
        .help("The name of the tool used to perform the verification"),
    )

  //
  // .arg(Arg::with_name("baseURL") // The base URL of your Pactflow account e.g. https://myaccount.pactflow.io
  // .arg(Arg::with_name("application") // The name of the provider API application
  // .arg(Arg::with_name("version") // The version of the provider API application
  // .arg(Arg::with_name("content") // The base64 encoded contents of the OAS (see base64 encoding below)
  // .arg(Arg::with_name("contractType")
  // .arg(Arg::with_name("content_type") // we can determine accept either so this doesn't matter
  // .arg(Arg::with_name("verificationResults")
  // .arg(Arg::with_name("verificationResults.success")
  // .arg(Arg::with_name("verificationResults.content")
  // .arg(Arg::with_name("verificationResults.contentType")
  // .arg(Arg::with_name("verificationResults.verifier")
}
fn handle_cli() -> Result<(), i32> {
  let args: Vec<String> = env::args().collect();
  let program = args[0].clone();
  let app = setup_app(&program, clap::crate_version!());
  let matches = app
    .setting(AppSettings::ArgRequiredElseHelp)
    .setting(AppSettings::ColoredHelp)
    .get_matches_safe();

  match matches {
    Ok(results) => handle_matches(&results),
    Err(ref err) => match err.kind {
      ErrorKind::HelpDisplayed => {
        println!("{}", err.message);
        Ok(())
      }
      ErrorKind::VersionDisplayed => Ok(()),
      _ => err.exit(),
    },
  }
}

fn handle_matches(args: &ArgMatches) -> Result<(), i32> {
  let log_level = args.value_of("loglevel");
  if let Err(err) = setup_loggers(log_level.unwrap_or("warn")) {
    eprintln!("WARN: Could not setup loggers: {}", err);
    eprintln!();
  }
  // println!("{:?}", args);
  // let mut sources: Vec<(String, anyhow::Result<Value>)> = vec![];
  // if let Some(values) = args.values_of("contentFile") {
  //   sources.extend(
  //     values
  //       .map(|v| (v.to_string(), load_file(v)))
  //       .collect::<Vec<(String, anyhow::Result<Value>)>>(),
  //   );
  // };

  let _files = load_files(args).map_err(|_| 1)?;
  let content_file = &_files[0];
  let report_file = &_files[1];
  println!("Content file: \n\n\n{:?}", content_file);
  println!("Report file: \n\n\n{:?}", report_file);
  Err(1)
}

fn load_files(args: &ArgMatches) -> anyhow::Result<Vec<(String, Value)>> {
  let mut sources: Vec<(String, anyhow::Result<Value>)> = vec![];
  if let Some(values) = args.values_of("contentFile") {
    sources.extend(
      values
        .map(|v| (v.to_string(), load_file(v)))
        .collect::<Vec<(String, anyhow::Result<Value>)>>(),
    );
  };
  if let Some(values) = args.values_of("resultsFile") {
    sources.extend(
      values
        .map(|v| (v.to_string(), load_file(v)))
        .collect::<Vec<(String, anyhow::Result<Value>)>>(),
    );
  };

  if sources.iter().any(|(_, res)| res.is_err()) {
    error!("Failed to load the following provider contracts:");
    for (source, result) in sources.iter().filter(|(_, res)| res.is_err()) {
      error!("    '{}' - {}", source, result.as_ref().unwrap_err());
    }
    Err(anyhow!("Failed to load one or more provider contracts"))
  } else {
    Ok(
      sources
        .iter()
        .map(|(source, result)| (source.clone(), result.as_ref().unwrap().clone()))
        .collect(),
    )
  }
}

fn load_file(file_name: &str) -> anyhow::Result<Value> {
  let file = File::open(file_name)?;
  let file_contents = serde_yaml::from_reader(file).context("file is not JSON or YML");
  // println!("{:?}", file_contents);
  file_contents
}

fn main() {
  match handle_cli() {
    Ok(_) => (),
    Err(err) => std::process::exit(err),
  }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderContractUploadRequestBody {
  pub content: String,
  pub contract_type: String,
  pub content_type: String,
  pub verification_results: VerificationResults,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResults {
  pub success: String,
  pub content: String,
  pub content_type: String,
  pub verifier: String,
}
