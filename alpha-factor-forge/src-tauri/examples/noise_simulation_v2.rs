//! Release-build runner for declared `research-noise-simulation-v2` runs
//! (docs/research-noise-simulation-v2.md §8, recalibration plan §8).
//!
//! ```text
//! cargo run --release --locked --example noise_simulation_v2 -- [--timing-only] <runs.json>
//! ```
//!
//! `<runs.json>` is either one declaration, or `{"runs": [{"id": "...",
//! "declaration": {...}}, ...]}`. Reports are printed to stdout as
//! `{"runs": [{"id", "report"}]}` in the input order; progress and timings go
//! to stderr. Runs execute in parallel, one thread each up to the machine's
//! parallelism; a report does not depend on that.
//!
//! `--timing-only` prints how long each run took and how much work it was,
//! and nothing about its outcome. It exists so the cost of a grid can be
//! measured before any of its results is looked at.
//!
//! Not a product path: nothing in the desktop or the service calls this.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use alpha_factor_forge::discovery_core::noise_simulation_v2::{
    parse_noise_simulation_v2, simulate_noise_v2, NoiseSimulationV2Declaration,
};
use serde_json::{json, Value};

/// A finished run: its report, or why it could not be simulated.
type Outcome = Result<Value, String>;

struct Run {
    id: String,
    declaration: NoiseSimulationV2Declaration,
}

/// Bars summed by the bootstrap over the whole run: the unit cost scales with.
fn bar_resamples(declaration: &NoiseSimulationV2Declaration) -> u128 {
    u128::from(declaration.simulations)
        * declaration.allocation.schedule.len() as u128
        * u128::from(declaration.candidates_per_confirmation)
        * u128::from(declaration.bootstrap_samples)
        * u128::from(declaration.bars)
}

fn load(path: &str) -> Result<Vec<Run>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let document: Value =
        serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))?;
    let entries: Vec<(String, &Value)> = match document.get("runs") {
        Some(Value::Array(runs)) => runs
            .iter()
            .enumerate()
            .map(|(index, run)| {
                let id = run["id"]
                    .as_str()
                    .map_or_else(|| format!("run-{index}"), str::to_string);
                (id, &run["declaration"])
            })
            .collect(),
        Some(_) => return Err(format!("{path}: \"runs\" must be an array")),
        None => vec![("run-0".to_string(), &document)],
    };
    entries
        .into_iter()
        .map(|(id, raw)| {
            parse_noise_simulation_v2(raw)
                .map(|declaration| Run {
                    id: id.clone(),
                    declaration,
                })
                .map_err(|error| format!("{id}: {error}"))
        })
        .collect()
}

fn main() {
    let mut timing_only = false;
    let mut path = None;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--timing-only" => timing_only = true,
            other if path.is_none() => path = Some(other.to_string()),
            other => fail(&format!("unexpected argument {other:?}")),
        }
    }
    let Some(path) = path else {
        fail("usage: noise_simulation_v2 [--timing-only] <runs.json>");
    };
    let runs = load(&path).unwrap_or_else(|message| fail(&message));

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Outcome>>> = Mutex::new(runs.iter().map(|_| None).collect());
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(runs.len())
        .max(1);
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(run) = runs.get(index) else {
                    break;
                };
                let clock = Instant::now();
                let outcome = simulate_noise_v2(&run.declaration)
                    .map(|report| serde_json::to_value(report).expect("a serializable report"))
                    .map_err(|error| error.to_string());
                let elapsed = clock.elapsed();
                eprintln!(
                    "{}: {:.3} s for {} bar-resamples",
                    run.id,
                    elapsed.as_secs_f64(),
                    bar_resamples(&run.declaration)
                );
                results.lock().unwrap()[index] = Some(outcome);
            });
        }
    });
    eprintln!(
        "{} run(s) on {workers} thread(s): {:.3} s wall",
        runs.len(),
        started.elapsed().as_secs_f64()
    );

    let mut reports = Vec::with_capacity(runs.len());
    for (run, result) in runs.iter().zip(results.into_inner().unwrap()) {
        match result.expect("every run finished") {
            Ok(report) => reports.push(json!({"id": run.id, "report": report})),
            Err(message) => fail(&format!("{}: {message}", run.id)),
        }
    }
    if !timing_only {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "runs": reports })).expect("serializable")
        );
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
