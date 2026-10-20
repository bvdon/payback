//! Development server: serves the built UI from `dist/` and exposes the core
//! API at `/api/*`, so the app can be developed and tested in any browser
//! without the desktop shell. Binds to localhost only.
//!
//!   cargo run -p payback-devserver -- --data ./payback-dev.json --dist ./dist --port 1422

use payback_core::api::{error_json, Api};
use payback_core::{Json, Store};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_BODY: usize = 64 * 1024 * 1024;

struct Config {
    data: PathBuf,
    dist: PathBuf,
    port: u16,
    fixed_now: Option<i64>,
}

fn parse_args() -> Config {
    let mut cfg =
        Config { data: PathBuf::from("payback-dev.json"), dist: PathBuf::from("dist"), port: 1422, fixed_now: None };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => cfg.data = args.next().map(PathBuf::from).unwrap_or(cfg.data),
            "--dist" => cfg.dist = args.next().map(PathBuf::from).unwrap_or(cfg.dist),
            "--port" => cfg.port = args.next().and_then(|p| p.parse().ok()).unwrap_or(cfg.port),
            // Freeze the clock (handy for demos and UI tests).
            "--now" => cfg.fixed_now = args.next().and_then(|p| p.parse().ok()),
            "-h" | "--help" => {
                println!("payback-devserver [--data FILE] [--dist DIR] [--port N] [--now UNIX_SECONDS]");
                std::process::exit(0);
            }
            other => eprintln!("ignoring unknown argument {other}"),
        }
    }
    cfg
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
    }
    if len > MAX_BODY {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "body too large"));
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    Ok(Request { method, path, body })
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, extra: &[(&str, String)], body: &[u8]) {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n",
        body.len()
    );
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "json" => "application/json",
        "woff2" => "font/woff2",
        "ico" => "image/x-icon",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

fn serve_static(stream: &mut TcpStream, dist: &Path, url_path: &str) {
    let clean = url_path.split(['?', '#']).next().unwrap_or("/");
    let rel = clean.trim_start_matches('/');
    if rel.split('/').any(|seg| seg == "..") {
        respond(stream, "400 Bad Request", "text/plain", &[], b"bad path");
        return;
    }
    let mut file = if rel.is_empty() { dist.join("index.html") } else { dist.join(rel) };
    if !file.is_file() {
        file = dist.join("index.html"); // single-page app fallback
    }
    match std::fs::read(&file) {
        Ok(bytes) => respond(stream, "200 OK", mime(&file), &[], &bytes),
        Err(_) => respond(
            stream,
            "404 Not Found",
            "text/plain",
            &[],
            format!("No UI build found in {}. Run `npm run build` first.", dist.display()).as_bytes(),
        ),
    }
}

fn handle(api: &mut Api, cfg: &Config, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let req = match read_request(&mut stream) {
        Ok(r) => r,
        Err(_) => return,
    };
    let path = req.path.split('?').next().unwrap_or("").to_string();
    if req.method == "GET" {
        serve_static(&mut stream, &cfg.dist, &req.path);
        return;
    }
    if req.method != "POST" {
        respond(&mut stream, "405 Method Not Allowed", "text/plain", &[], b"");
        return;
    }
    let body = String::from_utf8_lossy(&req.body).to_string();
    if let Some(cmd) = path.strip_prefix("/api/call/") {
        match api.call(cmd, &body) {
            Ok(json) => respond(&mut stream, "200 OK", "application/json", &[], json.as_bytes()),
            Err(err) => respond(&mut stream, "400 Bad Request", "application/json", &[], err.as_bytes()),
        }
    } else if let Some(cmd) = path.strip_prefix("/api/export/") {
        let result = Json::parse(&body).and_then(|args| api.export(cmd, &args));
        match result {
            Ok(x) => respond(
                &mut stream,
                "200 OK",
                x.mime,
                &[("Content-Disposition", format!("attachment; filename=\"{}\"", x.file_name))],
                &x.bytes,
            ),
            Err(e) => respond(&mut stream, "400 Bad Request", "application/json", &[], error_json(&e).as_bytes()),
        }
    } else if path == "/api/backup" {
        let dir = cfg.data.parent().unwrap_or(Path::new(".")).join("backups");
        match api.store().backup_now(&dir) {
            Ok(p) => respond(
                &mut stream,
                "200 OK",
                "application/json",
                &[],
                Json::Str(p.display().to_string()).stringify().as_bytes(),
            ),
            Err(e) => respond(&mut stream, "400 Bad Request", "application/json", &[], error_json(&e).as_bytes()),
        }
    } else {
        respond(&mut stream, "404 Not Found", "text/plain", &[], b"unknown endpoint");
    }
}

fn main() {
    let cfg = parse_args();
    let mut store = match Store::open(&cfg.data) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("could not open {}: {e}", cfg.data.display());
            std::process::exit(1);
        }
    };
    store.set_now(cfg.fixed_now);
    let mut api = Api::new(store);
    let listener = match TcpListener::bind(("127.0.0.1", cfg.port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("could not listen on 127.0.0.1:{}: {e}", cfg.port);
            std::process::exit(1);
        }
    };
    println!(
        "Payback dev server on http://127.0.0.1:{}  (data: {}, ui: {})",
        cfg.port,
        cfg.data.display(),
        cfg.dist.display()
    );
    for stream in listener.incoming().flatten() {
        handle(&mut api, &cfg, stream);
    }
}
