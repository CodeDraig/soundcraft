//! Desktop control and MCP command-line entry points.

use soundcraft_automation::Backend;

use super::{arg_value, fail};
use serde_json::Value;
use serde_json::json;
use soundcraft_automation::Headless;
use soundcraft_automation::Remote;
use soundcraft_automation::mcp::Server;
use soundcraft_engine::Engine;
use std::io::Write;
use std::process::ExitCode;

pub(super) fn app(args: &[String]) -> ExitCode {
    let port = arg_value(args, "--port").or_else(|| std::env::var("SOUNDCRAFT_CONTROL_PORT").ok()).unwrap_or_else(|| "7979".into());
    let positional: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !(a.starts_with("--") || *i > 0 && args.get(i - 1).is_some_and(|p| p == "--port")))
        .map(|(_, a)| a)
        .collect();
    let Some(method) = positional.first() else { return fail("app [--port P] METHOD [JSON]") };
    let params = positional.get(1).and_then(|p| serde_json::from_str::<Value>(p).ok()).unwrap_or(json!({}));
    let mut r = Remote::new(&port);
    // A bare command id is shorthand for engine.execute.
    let (m, p) = if method.contains('.')
        && !method.starts_with("ui.")
        && !method.starts_with("engine.")
        && !method.starts_with("session.")
        && !method.starts_with("app.")
    {
        ("engine.execute".to_string(), json!({"command": method, "params": params}))
    } else {
        (method.to_string(), params)
    };
    match r.call(&m, p) {
        Ok(v) => {
            outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

pub(super) fn mcp(args: &[String]) -> ExitCode {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let r = if let Some(port) = arg_value(args, "--connect") {
        let mut s = Server::new(Remote::new(&port));
        s.serve(stdin.lock(), stdout.lock())
    } else {
        let e = if args.iter().any(|a| a == "--demo") { soundcraft_engine::demo::demo_engine() } else { Engine::default() };
        let mut s = Server::new(Headless::new(e));
        s.serve(stdin.lock(), stdout.lock())
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}
