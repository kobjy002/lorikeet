use futures::StreamExt;
use structopt::StructOpt;

use std::path::{Path, PathBuf};

use anyhow::Error;

use log::{debug, trace};

use lorikeet::runner::run_steps;
use lorikeet::step::{ExpectType, Outcome, RetryPolicy, RunType, Step};
use lorikeet::submitter::StepResult;
use lorikeet::yaml::get_steps;

use std::time::Duration;

#[derive(StructOpt, Debug)]
#[structopt(name = "lorikeet", about = "a parallel test runner for devops")]
struct Arguments {
    #[structopt(short = "q", long = "quiet", help = "Don't output results to console")]
    quiet: bool,

    #[structopt(short = "c", long = "config", help = "Configuration File")]
    config: Option<String>,

    #[structopt(short = "h", long = "hostname", help = "Hostname")]
    hostname: Option<String>,

    #[structopt(short = "t", long = "terminal", help = "Force terminal colours")]
    term: bool,

    #[structopt(help = "Test Plan", default_value = "test.yml")]
    test_plan: String,

    #[structopt(
        short = "w",
        long = "webhook",
        help = "Webhook submission URL (multiple values allowed)"
    )]
    webhook: Vec<String>,

    #[structopt(
        short = "s",
        long = "slack",
        help = "Slack Webhook submission URL (multiple values allowed)"
    )]
    slack: Vec<String>,

    #[structopt(
        short = "j",
        long = "junit",
        help = "Output a JUnit XML Report to this file",
        parse(from_os_str)
    )]
    junit: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let opt = Arguments::from_args();

    env_logger::init();

    debug!("Loading Steps from `{}`", opt.test_plan);

    let mut has_errors = false;

    let colours = atty::is(atty::Stream::Stdout) || opt.term;

    // let results = run_steps_or_error(&opt.test_plan, &opt.config, opt.quiet, colours).await;
    let mut results = Vec::new();

    for step in run_steps_or_error(&opt.test_plan, &opt.config)
        .await
        .into_iter()
    {
        if let Some(ref outcome) = step.outcome {
            if outcome.error.is_some() {
                has_errors = true;
            }
        }
        let result = StepResult::from(step);
        if !opt.quiet {
            result.terminal_print(&colours);
        }
        results.push(result);
    }

    // let has_errors = results.iter().any(|val| !val.pass);

    debug!("Steps finished!");

    if !opt.webhook.is_empty() {
        let hostname = opt.hostname.clone().unwrap_or_else(|| {
            hostname::get()
                .map(|val| val.to_string_lossy().to_string())
                .unwrap_or_else(|_| "".into())
        });

        for url in opt.webhook {
            debug!("Sending webhook to: {}", url);
            lorikeet::submitter::submit_webhook(&results, &url, &hostname)
                .await
                .expect("Could not send webhook")
        }
    }

    if !opt.slack.is_empty() {
        let hostname = opt.hostname.unwrap_or_else(|| {
            hostname::get()
                .map(|val| val.to_string_lossy().to_string())
                .unwrap_or_else(|_| "".into())
        });

        for url in opt.slack {
            debug!("Sending slack webhook to: {}", url);
            lorikeet::submitter::submit_slack(&results, &url, &hostname)
                .await
                .expect("Could not send webhook")
        }
    }

    if let Some(path) = opt.junit {
        debug!("Creating junit file at `{}`", path.display());
        lorikeet::junit::create_junit(&results, &path, None).expect("Coult not create junit file");
    }

    if has_errors {
        std::process::exit(1)
    }
}

// Runs the steps, or if there is an issue running the steps, then return the error as a step
async fn run_steps_or_error<P: AsRef<Path>, Q: AsRef<Path>>(
    file_path: P,
    config_path: &Option<Q>,
//     quiet: bool,
//     colours: bool,
// ) -> Vec<StepResult> {
//     let steps = match get_steps(file_path, config_path) {
) -> Vec<Step> {
    let mut steps = match get_steps(file_path, config_path) {
        Ok(steps) => steps,
        // Err(err) => return vec![step_from_error(err, quiet, colours)],
        Err(err) => return vec![step_from_error(err)],
    };

    trace!("Steps:{:?}", steps);

    // match run_steps(steps) {
    //     Ok(mut stream) => {
    //         let mut results = Vec::new();

    //         while let Some(step) = stream.next().await {
    //             let result: StepResult = step.into();

    //             if !quiet {
    //                 result.terminal_print(&colours);
    //             }

    //             results.push(result);
    //         }

    //         results
    //     }
    //     Err(err) => vec![step_from_error(err, quiet, colours)],
    // }
    match run_steps(&mut steps).await {
        Ok(_) => steps,
        Err(err) => vec![step_from_error(err)],
    }
}

// fn step_from_error(err: Error, quiet: bool, colours: bool) -> StepResult {
fn step_from_error(err: Error) -> Step {
    let outcome = Outcome {
        output: None,
        error: Some(err.to_string()),
        duration: Duration::default(),
        on_fail_output: None,
        on_fail_error: None,
    };

    // let result: StepResult = Step {
    Step {
        name: "lorikeet".into(),
        run: RunType::Value(String::new()),
        do_output: true,
        expect: ExpectType::Anything,
        on_fail: None,
        description: Some(
            "This step is shown if there was an error when reading, parsing or running steps"
                .into(),
        ),
        filters: vec![],
        require: vec![],
        required_by: vec![],
        retry: RetryPolicy::default(),
        outcome: Some(outcome),
    }
    // .into();

    // if !quiet {
    //     result.terminal_print(&colours);
    // }

    // result
}
