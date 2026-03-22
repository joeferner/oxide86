use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::{DebugCommand, DebugResponse, DebugShared};

pub fn start_mcp_server(port: u16, debug: Arc<DebugShared>) {
    std::thread::spawn(move || {
        let server =
            Server::http(format!("127.0.0.1:{port}")).expect("MCP HTTP server failed to bind");
        log::info!("MCP debug server listening on http://127.0.0.1:{port}/mcp");
        for request in server.incoming_requests() {
            let debug = Arc::clone(&debug);
            std::thread::spawn(move || handle_request(request, debug));
        }
    });
}

fn handle_request(mut request: tiny_http::Request, debug: Arc<DebugShared>) {
    // CORS preflight
    if *request.method() == Method::Options {
        let _ = request.respond(
            Response::empty(StatusCode(204))
                .with_header(cors_header("Access-Control-Allow-Origin", "*"))
                .with_header(cors_header("Access-Control-Allow-Methods", "POST, OPTIONS"))
                .with_header(cors_header("Access-Control-Allow-Headers", "Content-Type")),
        );
        return;
    }

    if *request.method() != Method::Post {
        let _ = request.respond(Response::empty(StatusCode(405)));
        return;
    }

    // Read body
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        let _ = request.respond(Response::empty(StatusCode(400)));
        return;
    }

    let response_value = handle_message(&body, &debug);

    // Notifications produce a null value — respond 204 No Content
    if response_value.is_null() {
        let _ = request.respond(Response::empty(StatusCode(204)));
        return;
    }

    let body_out = serde_json::to_string(&response_value).unwrap_or_default();
    let _ = request.respond(
        Response::from_string(body_out)
            .with_header(cors_header("Content-Type", "application/json"))
            .with_header(cors_header("Access-Control-Allow-Origin", "*")),
    );
}

fn cors_header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn handle_message(line: &str, debug: &Arc<DebugShared>) -> Value {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": format!("Parse error: {e}") }
            });
        }
    };

    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req["method"].as_str().unwrap_or("");

    log::debug!("MCP handle message id:{id}, method:{method}");

    match method {
        "initialize" => handle_initialize(id),
        "notifications/initialized" | "initialized" => Value::Null,
        "tools/list" => handle_tools_list(id),
        "tools/call" => handle_tools_call(id, &req["params"], debug),
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        }),
    }
}

fn handle_initialize(id: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "oxide86-debugger", "version": "0.1.0" }
        }
    })
}

fn handle_tools_list(id: Value) -> Value {
    let no_params = json!({"type": "object", "properties": {}});
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                tool_def("get_registers", "Get all CPU registers from the latest snapshot", no_params.clone()),
                tool_def("get_status", "Get emulator status: running or paused_at CS:IP", no_params.clone()),
                tool_def("pause", "Pause the emulator after the current instruction", no_params.clone()),
                tool_def("continue", "Resume execution from a paused state", no_params.clone()),
                tool_def("step", "Execute N instructions while paused",
                    json!({"type":"object","properties":{"n":{"type":"integer","description":"Number of instructions (default 1)"}},"required":[]})),
                tool_def("set_breakpoint", "Add a CS:IP breakpoint",
                    json!({"type":"object","properties":{"seg":{"type":"string"},"off":{"type":"string"}},"required":["seg","off"]})),
                tool_def("clear_breakpoint", "Remove a CS:IP breakpoint",
                    json!({"type":"object","properties":{"seg":{"type":"string"},"off":{"type":"string"}},"required":["seg","off"]})),
                tool_def("list_breakpoints", "List all current breakpoints", no_params.clone()),
                tool_def("read_memory", "Read bytes from a flat physical address",
                    json!({"type":"object","properties":{"addr":{"type":"integer"},"len":{"type":"integer"}},"required":["addr","len"]})),
                tool_def("send_key", "Inject a PC scan code into the keyboard buffer",
                    json!({"type":"object","properties":{"scan_code":{"type":"integer"}},"required":["scan_code"]})),
                tool_def("set_write_watchpoint", "Pause when this physical address is written",
                    json!({"type":"object","properties":{"addr":{"type":"integer"}},"required":["addr"]})),
                tool_def("clear_write_watchpoint", "Remove a write watchpoint",
                    json!({"type":"object","properties":{"addr":{"type":"integer"}},"required":["addr"]})),
                tool_def("list_write_watchpoints", "List all write watchpoints", no_params.clone()),
                tool_def("get_fpu_registers", "Get FPU stack registers ST(0)–ST(7), control word, and status word", no_params.clone()),
            ]
        }
    })
}

fn handle_tools_call(id: Value, params: &Value, debug: &Arc<DebugShared>) -> Value {
    let tool = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];

    log::debug!("MCP handle tool call id:{id}, tool:{tool}, args:{args}");

    let result_text = match tool {
        "get_registers" => tool_get_registers(debug),
        "get_status" => tool_get_status(debug),
        "pause" => tool_pause(debug),
        "continue" => tool_continue(debug),
        "step" => tool_step(args, debug),
        "set_breakpoint" => tool_set_breakpoint(args, debug),
        "clear_breakpoint" => tool_clear_breakpoint(args, debug),
        "list_breakpoints" => tool_list_breakpoints(debug),
        "read_memory" => tool_read_memory(args, debug),
        "send_key" => tool_send_key(args, debug),
        "set_write_watchpoint" => tool_set_write_watchpoint(args, debug),
        "clear_write_watchpoint" => tool_clear_write_watchpoint(args, debug),
        "list_write_watchpoints" => tool_list_write_watchpoints(debug),
        "get_fpu_registers" => tool_get_fpu_registers(debug),
        _ => format!("Unknown tool: {tool}"),
    };
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": result_text }]
        }
    })
}

fn tool_def(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn tool_get_registers(debug: &Arc<DebugShared>) -> String {
    let snap = debug.snapshot.lock().unwrap();
    if let Some(s) = snap.as_ref() {
        format!(
            "CS={:04X} IP={:04X}\nAX={:04X} BX={:04X} CX={:04X} DX={:04X}\nSI={:04X} DI={:04X} SP={:04X} BP={:04X}\nDS={:04X} ES={:04X} SS={:04X} FS={:04X} GS={:04X}\nFLAGS={:04X}",
            s.cs,
            s.ip,
            s.ax,
            s.bx,
            s.cx,
            s.dx,
            s.si,
            s.di,
            s.sp,
            s.bp,
            s.ds,
            s.es,
            s.ss,
            s.fs,
            s.gs,
            s.flags
        )
    } else {
        "No snapshot available — emulator must be paused first".to_string()
    }
}

fn tool_get_status(debug: &Arc<DebugShared>) -> String {
    if debug.paused.load(Ordering::Relaxed) {
        // If paused by a write watchpoint, report the writing instruction's CS:IP
        if let Some(hit) = debug.watchpoint_hit.lock().unwrap().as_ref() {
            return format!(
                "paused_at {:04X}:{:04X} (watchpoint: wrote 0x{:02X} to 0x{:05X})",
                hit.2, hit.3, hit.1, hit.0
            );
        }
        let snap = debug.snapshot.lock().unwrap();
        if let Some(s) = snap.as_ref() {
            format!("paused_at {:04X}:{:04X}", s.cs, s.ip)
        } else {
            "paused".to_string()
        }
    } else {
        "running".to_string()
    }
}

fn tool_pause(debug: &Arc<DebugShared>) -> String {
    if debug.paused.load(Ordering::Relaxed) {
        return "Already paused".to_string();
    }
    debug.pause_requested.store(true, Ordering::SeqCst);
    // Wait until the emulator transitions to paused
    let lock = debug.snapshot.lock().unwrap();
    let guard = debug
        .cond_paused
        .wait_while(lock, |_| !debug.paused.load(Ordering::Relaxed))
        .unwrap();
    if let Some(s) = guard.as_ref() {
        format!("Paused at {:04X}:{:04X}", s.cs, s.ip)
    } else {
        "Paused".to_string()
    }
}

fn tool_continue(debug: &Arc<DebugShared>) -> String {
    if !debug.paused.load(Ordering::Relaxed) {
        return "Not paused".to_string();
    }
    debug.send_command(DebugCommand::Continue);
    "Resumed".to_string()
}

fn tool_step(args: &Value, debug: &Arc<DebugShared>) -> String {
    if !debug.paused.load(Ordering::Relaxed) {
        return "Not paused — use 'pause' first".to_string();
    }
    let n = args["n"].as_u64().unwrap_or(1) as u32;
    debug.send_command(DebugCommand::Step(n));
    let snap = debug.snapshot.lock().unwrap();
    if let Some(s) = snap.as_ref() {
        format!(
            "Stepped {n} instruction(s), now at {:04X}:{:04X}",
            s.cs, s.ip
        )
    } else {
        format!("Stepped {n} instruction(s)")
    }
}

fn tool_set_breakpoint(args: &Value, debug: &Arc<DebugShared>) -> String {
    match (parse_hex_arg(&args["seg"]), parse_hex_arg(&args["off"])) {
        (Some(seg), Some(off)) => {
            debug.add_breakpoint(seg, off);
            format!("Breakpoint set at {:04X}:{:04X}", seg, off)
        }
        _ => "Invalid arguments: seg and off must be hex strings or integers".to_string(),
    }
}

fn tool_clear_breakpoint(args: &Value, debug: &Arc<DebugShared>) -> String {
    match (parse_hex_arg(&args["seg"]), parse_hex_arg(&args["off"])) {
        (Some(seg), Some(off)) => {
            debug.remove_breakpoint(seg, off);
            format!("Breakpoint cleared at {:04X}:{:04X}", seg, off)
        }
        _ => "Invalid arguments: seg and off must be hex strings or integers".to_string(),
    }
}

fn tool_list_breakpoints(debug: &Arc<DebugShared>) -> String {
    let mut bps = debug.list_breakpoints();
    if bps.is_empty() {
        "No breakpoints set".to_string()
    } else {
        bps.sort();
        bps.iter()
            .map(|(s, o)| format!("{:04X}:{:04X}", s, o))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn tool_read_memory(args: &Value, debug: &Arc<DebugShared>) -> String {
    if !debug.paused.load(Ordering::Relaxed) {
        return "Not paused — use 'pause' first".to_string();
    }
    let addr = args["addr"].as_u64().map(|v| v as u32);
    let len = args["len"].as_u64().map(|v| v as u32);
    match (addr, len) {
        (Some(a), Some(l)) => {
            match debug.send_command(DebugCommand::ReadMemory { addr: a, len: l }) {
                DebugResponse::Memory(bytes) => format_hex_dump(a as usize, &bytes),
                DebugResponse::Ok => String::new(),
            }
        }
        _ => "Invalid arguments: addr and len must be integers".to_string(),
    }
}

fn tool_send_key(args: &Value, debug: &Arc<DebugShared>) -> String {
    match args["scan_code"].as_u64().map(|v| v as u8) {
        Some(sc) => {
            if debug.paused.load(Ordering::Relaxed) {
                debug.send_command(DebugCommand::SendKey(sc));
                format!("Key 0x{sc:02X} queued")
            } else {
                "Not paused — use 'pause' first, send key, then 'continue'".to_string()
            }
        }
        None => "Invalid argument: scan_code must be an integer".to_string(),
    }
}

fn tool_set_write_watchpoint(args: &Value, debug: &Arc<DebugShared>) -> String {
    match args["addr"].as_u64().map(|v| v as u32) {
        Some(a) => {
            debug.add_write_watchpoint(a);
            format!("Write watchpoint set at 0x{a:05X}")
        }
        None => "Invalid argument: addr must be an integer".to_string(),
    }
}

fn tool_clear_write_watchpoint(args: &Value, debug: &Arc<DebugShared>) -> String {
    match args["addr"].as_u64().map(|v| v as u32) {
        Some(a) => {
            debug.remove_write_watchpoint(a);
            format!("Write watchpoint cleared at 0x{a:05X}")
        }
        None => "Invalid argument: addr must be an integer".to_string(),
    }
}

fn tool_list_write_watchpoints(debug: &Arc<DebugShared>) -> String {
    let mut wps = debug.list_write_watchpoints();
    if wps.is_empty() {
        "No write watchpoints set".to_string()
    } else {
        wps.sort();
        wps.iter()
            .map(|a| format!("0x{a:05X}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn tool_get_fpu_registers(debug: &Arc<DebugShared>) -> String {
    let snap = debug.snapshot.lock().unwrap();
    if let Some(s) = snap.as_ref() {
        let top = s.fpu_top as usize;
        let mut lines = Vec::new();
        lines.push(format!(
            "TOP={} SW={:04X} CW={:04X}",
            top, s.fpu_status_word, s.fpu_control_word
        ));
        for i in 0..8usize {
            let phys = (top + i) & 7;
            let val = s.fpu_stack[phys];
            lines.push(format!("ST({i}) [{phys}] = {val:e}  ({val})"));
        }
        lines.join("\n")
    } else {
        "No snapshot available — pause the emulator first".to_string()
    }
}

fn format_hex_dump(base: usize, bytes: &[u8]) -> String {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(i, row)| {
            let offset = base + i * 16;
            let hex: String = row.iter().map(|b| format!("{b:02X} ")).collect();
            let ascii: String = row
                .iter()
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            format!("{offset:05X}  {hex:<48}  {ascii}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse a hex string (with or without 0x) or integer JSON value into a u16.
fn parse_hex_arg(v: &Value) -> Option<u16> {
    if let Some(s) = v.as_str() {
        let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
        u16::from_str_radix(s, 16).ok()
    } else if let Some(n) = v.as_u64() {
        u16::try_from(n).ok()
    } else {
        None
    }
}
