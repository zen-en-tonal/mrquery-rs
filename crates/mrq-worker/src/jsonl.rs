use std::io::{BufRead, Write};

use crate::{
    commands::{
        handle_apply_batch, handle_create_snapshot, handle_query, handle_reload, WorkerState,
    },
    protocol::{RequestEnvelope, ResponseEnvelope},
};

pub fn run_loop(state: &mut WorkerState) {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("stdin read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }

        let response = handle_line(state, &line);
        let json = match serde_json::to_string(&response) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("serialize response error: {e}");
                continue;
            }
        };
        if let Err(e) = writeln!(out, "{}", json) {
            tracing::error!("stdout write error: {e}");
            break;
        }
        let _ = out.flush();
    }
}

fn handle_line(state: &mut WorkerState, line: &str) -> ResponseEnvelope {
    let req: RequestEnvelope = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return ResponseEnvelope::err("unknown".to_string(), "parse_error", e.to_string());
        }
    };

    let rid = req.request_id.clone();
    let result = match req.command.as_str() {
        "query" => handle_query(state, &req.payload),
        "apply_batch" => handle_apply_batch(state, &req.payload),
        "create_snapshot" => handle_create_snapshot(state, &req.payload),
        "reload" => handle_reload(state),
        other => Err(anyhow::anyhow!("Unknown command: {other}")),
    };

    match result {
        Ok(payload) => ResponseEnvelope::ok(rid, payload),
        Err(e) => ResponseEnvelope::err(rid, "error", e.to_string()),
    }
}
