//! `ws-move-loadtest` — see `README.md` for usage and baseline numbers.

use std::process::ExitCode;
use ws_move_loadtest::{
    format_report, mock, parse_args, run, Command, LoadTestConfig, DEFAULT_SELF_TEST_SECRET,
};

#[tokio::main]
async fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_args(&raw) {
        Ok(Command::Run(options)) => *options,
        Ok(Command::Help) => {
            print!("{}", ws_move_loadtest::HELP_TEXT);
            return ExitCode::SUCCESS;
        }
        Ok(Command::Version) => {
            println!("ws-move-loadtest {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("error: {}\n\n{}", error, ws_move_loadtest::HELP_TEXT);
            return ExitCode::from(2);
        }
    };

    let mut config: LoadTestConfig = options.config.clone();

    // Hold the mock server for the whole run; its Drop stops the listener.
    let _mock_server = if options.self_test {
        let secret = config
            .jwt_secret
            .clone()
            .unwrap_or_else(|| DEFAULT_SELF_TEST_SECRET.to_string());
        match mock::spawn(secret.clone()).await {
            Ok(server) => {
                println!("self-test: mock endpoint listening on {}", server.url);
                config.url = server.url.clone();
                config.jwt_secret = Some(secret);
                Some(server)
            }
            Err(error) => {
                eprintln!("error: could not start the mock endpoint: {}", error);
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };

    let report = run(config).await;
    print!("{}", format_report(&report));

    if let Some(path) = &options.json_path {
        let payload = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
        if let Err(error) = std::fs::write(path, payload) {
            eprintln!("error: could not write {}: {}", path.display(), error);
            return ExitCode::from(2);
        }
        println!("json report written to {}", path.display());
    }

    if report.exceeds_error_budget(options.max_error_rate) {
        eprintln!(
            "error rate {:.4} % exceeds the allowed budget {:.4} %",
            report.error_rate * 100.0,
            options.max_error_rate * 100.0
        );
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
