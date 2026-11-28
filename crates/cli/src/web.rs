//! `cargo parallax run web` and `build web`: the app's cdylib built for wasm32, wasm-bindgen's loader, a host page
//! from templates/web/, and (for `run`) a small HTTP server. Step by step in docs/cli-web.md. Decisions are pure
//! functions over plain values; `build` and `serve` are the thin shell.

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::android::{self, CARGO_FIX, Metadata, Profile};
use crate::config::{Config, Web};
use crate::error::CliError;

const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// Where `run web` listens without `--host` / `--port`: this machine only.
const DEFAULT_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);

/// Pages written from templates/web/ (`{{key}}` filled from `site_vars`). `index.html` is the app's own
/// `web.index` instead when set. To add a file: put it under templates/web/ and add a row.
const TEMPLATES: [(&str, &str); 2] = [
    ("index.html", include_str!("../templates/web/index.html")),
    (
        "manifest.json",
        include_str!("../templates/web/manifest.json"),
    ),
];

/// `app.icon`, copied as is: browsers draw SVG favicons and manifest icons.
const ICON: &str = "favicon.svg";

/// wasm-bindgen's output directory, next to index.html.
const PKG: &str = "pkg";

/// The WebAssembly features rustc's wasm32-unknown-unknown target turns on by default, which wasm-opt must accept.
/// To follow a new rustc default: add its wasm-opt flag.
const WASM_OPT_FEATURES: [&str; 6] = [
    "--enable-bulk-memory",
    "--enable-multivalue",
    "--enable-mutable-globals",
    "--enable-nontrapping-float-to-int",
    "--enable-reference-types",
    "--enable-sign-ext",
];

/// What each cargo profile does after `cargo build`.
struct WebProfile {
    /// wasm-bindgen flags: `--debug` keeps its JS glue's checks and names, as wasm-pack's dev profile does.
    bindgen: &'static [&'static str],
    /// Run wasm-opt `-O` when it is on PATH.
    optimise: bool,
}

fn web_profile(profile: Profile) -> WebProfile {
    match profile {
        Profile::Debug => WebProfile {
            bindgen: &["--debug"],
            optimise: false,
        },
        Profile::Release => WebProfile {
            bindgen: &[],
            optimise: true,
        },
    }
}

/// Host OS → the command that opens a URL in the default browser. To add a host: add a row.
const OPENERS: [(&str, &str, &[&str]); 3] = [
    ("linux", "xdg-open", &[]),
    ("macos", "open", &[]),
    ("windows", "cmd", &["/C", "start", ""]),
];

/// File extension → `Content-Type`. Browsers compile `.wasm` streaming only as `application/wasm`. To serve a new
/// kind: add a row.
const MIME_TYPES: [(&str, &str); 18] = [
    ("html", "text/html; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("mjs", "text/javascript; charset=utf-8"),
    ("wasm", "application/wasm"),
    ("json", "application/json"),
    ("webmanifest", "application/manifest+json"),
    ("css", "text/css; charset=utf-8"),
    ("txt", "text/plain; charset=utf-8"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
    ("ico", "image/x-icon"),
    ("ttf", "font/ttf"),
    ("otf", "font/otf"),
    ("woff2", "font/woff2"),
    ("mp4", "video/mp4"),
];
const DEFAULT_MIME: &str = "application/octet-stream";

struct WebArgs {
    profile: Profile,
    addr: SocketAddr,
    open: bool,
}

/// `run web [--release] [--port <n>] [--host <address>] [--open]`.
pub fn run(app_dir: &Path, config: &Config, args: &[String]) -> Result<(), CliError> {
    let args = parse_args(args, true)?;
    let dist = build_dist(app_dir, config, args.profile)?;
    serve(&dist, args.addr, args.open)
}

/// `build web [--release]`.
pub fn build(app_dir: &Path, config: &Config, args: &[String]) -> Result<(), CliError> {
    let args = parse_args(args, false)?;
    build_dist(app_dir, config, args.profile).map(|_| ())
}

/// Flags after `run web` (`serving`) or `build web`, which takes only `--release`.
fn parse_args(args: &[String], serving: bool) -> Result<WebArgs, CliError> {
    let mut out = WebArgs {
        profile: Profile::Debug,
        addr: DEFAULT_ADDR,
        open: false,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = |what: &str| {
            rest.next()
                .ok_or_else(|| CliError::Usage(format!("{arg} needs {what}")))
        };
        match (arg.as_str(), serving) {
            ("--release", _) => out.profile = Profile::Release,
            ("--port", true) => {
                let port = value("a port number")?;
                out.addr.set_port(port.parse().map_err(|_| {
                    CliError::Usage(format!("--port {port:?} is not a port number"))
                })?);
            }
            ("--host", true) => {
                let host = value("an IP address")?;
                out.addr.set_ip(host.parse().map_err(|_| {
                    CliError::Usage(format!(
                        "--host {host:?} is not an IP address (0.0.0.0 serves the LAN)"
                    ))
                })?);
            }
            ("--open", true) => out.open = true,
            (other, _) => return Err(CliError::Usage(format!("unknown argument {other:?}"))),
        }
    }
    Ok(out)
}

/// Builds the app and writes the deployable site to `<target dir>/parallax/web/<package>/`; returns that directory.
fn build_dist(app_dir: &Path, config: &Config, profile: Profile) -> Result<PathBuf, CliError> {
    let manifest = app_dir.join("Cargo.toml");
    let metadata = android::metadata(&manifest)?;
    let (package, lib) = android::app_package(&metadata, &manifest)?;
    // Fail on a missing or mismatched wasm-bindgen before the long build.
    let want = bindgen_version(&metadata, &package.name)?;
    let have = match Command::new("wasm-bindgen").arg("--version").output() {
        Ok(out) => cli_version(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => None,
    };
    if have.as_deref() != Some(want.as_str()) {
        return Err(CliError::WasmBindgen { have, want });
    }

    let read = |rel: &Path| {
        let path = app_dir.join(rel);
        std::fs::read(&path).map_err(|source| CliError::Io { path, source })
    };
    let index = match &config.web.index {
        Some(path) => Some(String::from_utf8_lossy(&read(path)?).into_owned()),
        None => None,
    };
    let icon = match &config.icon {
        Some(path) => {
            Some(
                std::fs::read(app_dir.join(path)).map_err(|source| CliError::IconRead {
                    path: app_dir.join(path),
                    source,
                })?,
            )
        }
        None => None,
    };
    let assets = config
        .web
        .assets
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                .ok_or_else(|| CliError::AssetName(path.clone()))?;
            Ok((name.to_string_lossy().into_owned(), read(path)?))
        })
        .collect::<Result<Vec<_>, CliError>>()?;
    let files = site_files(&config.web, &lib, index, icon, assets)?;

    let web = web_profile(profile);
    android::step(
        Command::new(android::cargo())
            .args(["build", "--lib", "--target", WASM_TARGET, "--manifest-path"])
            .arg(&manifest)
            .args(profile.cargo_args()),
        CARGO_FIX,
        "cargo build",
        format!("If the target is missing: rustup target add {WASM_TARGET}"),
    )?;

    let dist = metadata
        .target_directory
        .join("parallax/web")
        .join(&package.name);
    if dist.exists() {
        std::fs::remove_dir_all(&dist).map_err(|source| CliError::Io {
            path: dist.clone(),
            source,
        })?;
    }
    let wasm = metadata
        .target_directory
        .join(WASM_TARGET)
        .join(profile.dir())
        .join(format!("{lib}.wasm"));
    let pkg = dist.join(PKG);
    android::step(
        Command::new("wasm-bindgen")
            .args(["--target", "web", "--no-typescript", "--out-name", &lib])
            .args(web.bindgen)
            .arg("--out-dir")
            .arg(&pkg)
            .arg(&wasm),
        BINDGEN_FIX,
        "wasm-bindgen",
        String::new(),
    )?;
    if web.optimise {
        optimise(&pkg.join(format!("{lib}_bg.wasm")))?;
    }
    for (path, bytes) in &files {
        android::write(&dist.join(path), bytes)?;
    }
    println!("cargo parallax: web app in {}", dist.display());
    Ok(dist)
}

const BINDGEN_FIX: &str = "Install wasm-bindgen-cli (see the version check above)";

/// wasm-opt `-O` in place, when binaryen's wasm-opt is on PATH; without it the release build stays unoptimised.
fn optimise(wasm: &Path) -> Result<(), CliError> {
    let out = wasm.with_extension("opt.wasm");
    let mut cmd = Command::new("wasm-opt");
    cmd.arg("-O")
        .args(WASM_OPT_FEATURES)
        .arg(wasm)
        .arg("-o")
        .arg(&out);
    match cmd.status() {
        Ok(status) if status.success() => {
            std::fs::rename(&out, wasm).map_err(|source| CliError::Io {
                path: wasm.to_path_buf(),
                source,
            })
        }
        Ok(status) => Err(CliError::StepFailed {
            step: "wasm-opt",
            status,
            hint: String::new(),
        }),
        Err(_) => {
            println!(
                "cargo parallax: wasm-opt not found, so the .wasm is not size-optimised. Install binaryen to optimise"
            );
            Ok(())
        }
    }
}

/// The wasm-bindgen crate version in the app's lock file. wasm-bindgen-cli must be exactly this version.
fn bindgen_version(metadata: &Metadata, package: &str) -> Result<String, CliError> {
    let mut versions: Vec<&str> = metadata
        .packages
        .iter()
        .filter(|p| p.name == "wasm-bindgen")
        .map(|p| p.version.as_str())
        .collect();
    versions.sort_unstable();
    versions.dedup();
    match versions[..] {
        [] => Err(CliError::NoWasmBindgen(package.to_string())),
        [one] => Ok(one.to_string()),
        _ => Err(CliError::SeveralWasmBindgens(
            versions.iter().map(|v| v.to_string()).collect(),
        )),
    }
}

/// `0.2.130` from `wasm-bindgen --version`'s `wasm-bindgen 0.2.130`.
fn cli_version(stdout: &str) -> Option<String> {
    stdout
        .trim()
        .strip_prefix("wasm-bindgen ")
        .map(|v| v.split_whitespace().next().unwrap_or(v).to_string())
}

/// Every file of the site but `pkg/`: (path in the site, bytes). Pure: the shell reads the inputs.
fn site_files(
    web: &Web,
    lib: &str,
    index: Option<String>,
    icon: Option<Vec<u8>>,
    assets: Vec<(String, Vec<u8>)>,
) -> Result<Vec<(String, Vec<u8>)>, CliError> {
    let vars = site_vars(web, lib, icon.is_some());
    let mut files: Vec<(String, Vec<u8>)> = TEMPLATES
        .iter()
        .map(|(path, template)| {
            let text = match (*path, &index) {
                ("index.html", Some(own)) => own.as_str(),
                _ => template,
            };
            (path.to_string(), android::render(text, &vars).into_bytes())
        })
        .collect();
    if let Some(svg) = icon {
        files.push((ICON.into(), svg));
    }
    for (name, bytes) in assets {
        if name == PKG || files.iter().any(|(path, _)| *path == name) {
            return Err(CliError::AssetClash(name));
        }
        files.push((name, bytes));
    }
    Ok(files)
}

/// `{{key}}` values for the templates (and the app's own `web.index`), escaped for where each lands.
fn site_vars(web: &Web, lib: &str, icon: bool) -> Vec<(&'static str, String)> {
    let (icon_links, icons_json) = match icon {
        true => (
            format!(r#"  <link rel="icon" type="image/svg+xml" href="{ICON}">"#),
            format!(r#"[{{ "src": "{ICON}", "sizes": "any", "type": "image/svg+xml" }}]"#),
        ),
        false => (String::new(), "[]".into()),
    };
    vec![
        ("title", android::xml_escape(&web.title)),
        (
            "title_json",
            serde_json::Value::from(web.title.as_str()).to_string(),
        ),
        // Checked to be #rgb or #rrggbb (config.rs), so it needs no escaping.
        ("background", web.background.clone()),
        ("lib", lib.to_string()),
        ("icon_links", icon_links),
        ("icons_json", icons_json),
    ]
}

/// Serves `dist` until the process is stopped, one thread per connection.
fn serve(dist: &Path, addr: SocketAddr, open: bool) -> Result<(), CliError> {
    let listener = TcpListener::bind(addr).map_err(|source| CliError::Bind { addr, source })?;
    let url = page_url(addr);
    println!(
        "cargo parallax: serving {} at {url} (Ctrl-C stops)",
        dist.display()
    );
    if open {
        open_browser(&url);
    }
    for stream in listener.incoming().flatten() {
        let dist = dist.to_path_buf();
        std::thread::spawn(move || respond(stream, &dist));
    }
    Ok(())
}

/// The URL to print and open: an unspecified address (0.0.0.0, ::) is reached through loopback on this machine.
fn page_url(addr: SocketAddr) -> String {
    let ip = match addr.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    format!("http://{}/", SocketAddr::new(ip, addr.port()))
}

/// Opens `url` in the default browser; a failure only prints, the server keeps running.
fn open_browser(url: &str) {
    let opened = match OPENERS
        .iter()
        .find(|(os, _, _)| *os == std::env::consts::OS)
    {
        Some((_, program, args)) => Command::new(program)
            .args(*args)
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("{program}: {e}")),
        None => Err(format!("no opener for {}", std::env::consts::OS)),
    };
    if let Err(why) = opened {
        println!("cargo parallax: cannot open a browser ({why}); open {url} yourself");
    }
}

/// What a request gets.
#[derive(Debug, PartialEq)]
enum Reply {
    /// A file under the site; `body: false` for HEAD.
    File {
        path: String,
        body: bool,
    },
    Error(Status),
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Status {
    Ok,
    BadRequest,
    NotFound,
    MethodNotAllowed,
}

impl Status {
    fn line(self) -> &'static str {
        match self {
            Status::Ok => "200 OK",
            Status::BadRequest => "400 Bad Request",
            Status::NotFound => "404 Not Found",
            Status::MethodNotAllowed => "405 Method Not Allowed",
        }
    }
}

/// An HTTP request line → the file it asks for. `/` and `…/` are `index.html`; `..`, `.` and empty segments and
/// backslashes are refused, so nothing outside the site is reachable. The query and fragment are ignored.
fn route(request_line: &str) -> Reply {
    let [method, target, _version] = request_line.split_whitespace().collect::<Vec<_>>()[..] else {
        return Reply::Error(Status::BadRequest);
    };
    let body = match method {
        "GET" => true,
        "HEAD" => false,
        _ => return Reply::Error(Status::MethodNotAllowed),
    };
    let path = target.split(['?', '#']).next().unwrap_or("");
    let Some(path) = path.strip_prefix('/') else {
        return Reply::Error(Status::BadRequest);
    };
    let path = match path.is_empty() || path.ends_with('/') {
        true => format!("{path}index.html"),
        false => path.to_string(),
    };
    let safe = !path.contains('\\') && path.split('/').all(|s| !["", ".", ".."].contains(&s));
    match safe {
        true => Reply::File { path, body },
        false => Reply::Error(Status::NotFound),
    }
}

fn mime_type(path: &str) -> &'static str {
    let extension = path.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    MIME_TYPES
        .iter()
        .find(|(e, _)| e.eq_ignore_ascii_case(extension))
        .map(|(_, mime)| *mime)
        .unwrap_or(DEFAULT_MIME)
}

/// Answers one connection: reads the request head, writes one response, closes. A client that hangs up early
/// only loses its own response.
fn respond(stream: TcpStream, dist: &Path) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    // The headers: read to the blank line so the client sees its request taken, then ignored.
    let mut header = String::new();
    while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
        header.clear();
    }
    let (status, mime, bytes, body) = match route(&request_line) {
        Reply::File { path, body } => match std::fs::read(dist.join(&path)) {
            Ok(bytes) => (Status::Ok, mime_type(&path), bytes, body),
            Err(_) => (Status::NotFound, DEFAULT_MIME, Vec::new(), body),
        },
        Reply::Error(status) => (status, DEFAULT_MIME, Vec::new(), true),
    };
    println!("{} → {}", request_line.trim_end(), status.line());
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        status.line(),
        bytes.len()
    );
    let mut stream = &stream;
    let _ = stream.write_all(head.as_bytes());
    if body {
        let _ = stream.write_all(&bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_table() {
        // (row name, flags, serving (run) or not (build), expected (profile dir, address, --open) or a usage error)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            bool,
            Option<(&'a str, &'a str, bool)>,
        );
        let cases: [Case; 14] = [
            (
                "run defaults",
                &[],
                true,
                Some(("debug", "127.0.0.1:8080", false)),
            ),
            (
                "build defaults",
                &[],
                false,
                Some(("debug", "127.0.0.1:8080", false)),
            ),
            (
                "run release",
                &["--release"],
                true,
                Some(("release", "127.0.0.1:8080", false)),
            ),
            (
                "build release",
                &["--release"],
                false,
                Some(("release", "127.0.0.1:8080", false)),
            ),
            (
                "port",
                &["--port", "9000"],
                true,
                Some(("debug", "127.0.0.1:9000", false)),
            ),
            (
                "LAN host",
                &["--host", "0.0.0.0"],
                true,
                Some(("debug", "0.0.0.0:8080", false)),
            ),
            (
                "IPv6 host",
                &["--host", "::1"],
                true,
                Some(("debug", "[::1]:8080", false)),
            ),
            (
                "every flag",
                &[
                    "--open",
                    "--port",
                    "1234",
                    "--release",
                    "--host",
                    "192.168.1.2",
                ],
                true,
                Some(("release", "192.168.1.2:1234", true)),
            ),
            ("port without number", &["--port"], true, None),
            ("port not a number", &["--port", "web"], true, None),
            ("port out of range", &["--port", "70000"], true, None),
            ("host a name", &["--host", "localhost"], true, None),
            ("build takes no port", &["--port", "9000"], false, None),
            ("unknown flag", &["--fast"], true, None),
        ];
        for (name, args, serving, want) in cases {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let got = parse_args(&args, serving)
                .map(|a| (a.profile.dir(), a.addr.to_string(), a.open))
                .map_err(|e| e.variant());
            let want = want
                .map(|(profile, addr, open)| (profile, addr.to_string(), open))
                .ok_or_else(|| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn route_table() {
        let file = |path: &str, body| Reply::File {
            path: path.into(),
            body,
        };
        let cases = [
            (
                "root is the page",
                "GET / HTTP/1.1",
                file("index.html", true),
            ),
            (
                "a file",
                "GET /pkg/hello_bg.wasm HTTP/1.1",
                file("pkg/hello_bg.wasm", true),
            ),
            (
                "query ignored",
                "GET /frame.html?name=same HTTP/1.1",
                file("frame.html", true),
            ),
            (
                "query on the root",
                "GET /?offset&animate HTTP/1.1",
                file("index.html", true),
            ),
            (
                "fragment ignored",
                "GET /index.html#x HTTP/1.1",
                file("index.html", true),
            ),
            (
                "directory is its index",
                "GET /docs/ HTTP/1.1",
                file("docs/index.html", true),
            ),
            (
                "HEAD has no body",
                "HEAD / HTTP/1.1",
                file("index.html", false),
            ),
            (
                "parent refused",
                "GET /../Cargo.toml HTTP/1.1",
                Reply::Error(Status::NotFound),
            ),
            (
                "nested parent refused",
                "GET /pkg/../../x HTTP/1.1",
                Reply::Error(Status::NotFound),
            ),
            (
                "dot segment refused",
                "GET /./index.html HTTP/1.1",
                Reply::Error(Status::NotFound),
            ),
            (
                "double slash refused",
                "GET //etc/passwd HTTP/1.1",
                Reply::Error(Status::NotFound),
            ),
            (
                "backslash refused",
                "GET /..\\x HTTP/1.1",
                Reply::Error(Status::NotFound),
            ),
            (
                "POST refused",
                "POST / HTTP/1.1",
                Reply::Error(Status::MethodNotAllowed),
            ),
            (
                "absolute URL refused",
                "GET http://x/ HTTP/1.1",
                Reply::Error(Status::BadRequest),
            ),
            ("garbage", "hello", Reply::Error(Status::BadRequest)),
            ("empty", "", Reply::Error(Status::BadRequest)),
        ];
        for (name, line, want) in cases {
            assert_eq!(route(line), want, "{name}");
        }
    }

    #[test]
    fn mime_type_table() {
        let cases = [
            ("pkg/hello_bg.wasm", "application/wasm"),
            ("pkg/hello.js", "text/javascript; charset=utf-8"),
            ("index.html", "text/html; charset=utf-8"),
            ("manifest.json", "application/json"),
            ("favicon.svg", "image/svg+xml"),
            ("FONT.TTF", "font/ttf"),
            ("README", DEFAULT_MIME),
            ("archive.tar.zst", DEFAULT_MIME),
        ];
        for (path, want) in cases {
            assert_eq!(mime_type(path), want, "{path}");
        }
    }

    #[test]
    fn page_url_table() {
        let cases = [
            ("127.0.0.1:8080", "http://127.0.0.1:8080/"),
            ("0.0.0.0:9000", "http://127.0.0.1:9000/"),
            ("[::]:80", "http://[::1]:80/"),
            ("192.168.1.2:8080", "http://192.168.1.2:8080/"),
        ];
        for (addr, want) in cases {
            let addr: SocketAddr = addr.parse().expect("test address");
            assert_eq!(page_url(addr), want, "{addr}");
        }
    }

    fn web() -> Web {
        Web {
            title: "Tom & \"Jerry\"".into(),
            background: "#102030".into(),
            index: None,
            assets: Vec::new(),
        }
    }

    #[test]
    fn site_files_table() {
        let own = "<title>{{title}}</title><script type=module>import init from \"./pkg/{{lib}}.js\"; `${x}`</script>";
        type Case<'a> = (
            &'a str,
            Option<&'a str>,
            bool,
            &'a [&'a str],
            Result<&'a [&'a str], &'a str>,
        );
        // (row name, own index, icon, asset names, expected file paths or error variant)
        let cases: [Case; 6] = [
            (
                "template, no icon",
                None,
                false,
                &[],
                Ok(&["index.html", "manifest.json"]),
            ),
            (
                "icon",
                None,
                true,
                &[],
                Ok(&["index.html", "manifest.json", "favicon.svg"]),
            ),
            (
                "own index and an asset",
                Some(own),
                false,
                &["frame.html"],
                Ok(&["index.html", "manifest.json", "frame.html"]),
            ),
            (
                "asset named index.html",
                None,
                false,
                &["index.html"],
                Err("AssetClash"),
            ),
            (
                "asset named like the icon",
                None,
                true,
                &["favicon.svg"],
                Err("AssetClash"),
            ),
            ("asset named pkg", None, false, &["pkg"], Err("AssetClash")),
        ];
        for (name, index, icon, assets, want) in cases {
            let assets = assets
                .iter()
                .map(|a| (a.to_string(), b"x".to_vec()))
                .collect();
            let icon = icon.then(|| b"<svg/>".to_vec());
            let got = site_files(&web(), "hello", index.map(String::from), icon, assets);
            let paths = got
                .as_ref()
                .map(|files| files.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>())
                .map_err(|e| e.variant());
            let want = want.map(|paths| paths.to_vec()).map_err(String::from);
            assert_eq!(paths, want, "{name}");
            for (path, bytes) in got.iter().flatten() {
                let text = String::from_utf8_lossy(bytes);
                assert!(!text.contains("{{"), "{name}: {path} keeps a placeholder");
                assert!(
                    !text.contains("Tom & \""),
                    "{name}: {path} has the title unescaped"
                );
            }
        }
    }

    #[test]
    fn template_contents() -> Result<(), CliError> {
        let page = |icon: bool| -> Result<(String, String), CliError> {
            let files = site_files(&web(), "hello", None, icon.then(Vec::new), Vec::new())?;
            let text = |i: usize| String::from_utf8_lossy(&files[i].1).into_owned();
            Ok((text(0), text(1)))
        };
        let (index, manifest) = page(true)?;
        // (row name, file, text it must hold)
        let cases = [
            (
                "loader imports the cdylib's glue",
                &index,
                r#"import init from "./pkg/hello.js";"#,
            ),
            (
                "viewport covers the safe area",
                &index,
                "viewport-fit=cover",
            ),
            (
                "title escaped for HTML",
                &index,
                "<title>Tom &amp; &quot;Jerry&quot;</title>",
            ),
            ("background in the page", &index, "background: #102030;"),
            ("favicon linked", &index, r#"href="favicon.svg""#),
            (
                "Flutter credit",
                &index,
                "Copyright 2014 The Flutter Authors",
            ),
            (
                "manifest name is JSON",
                &manifest,
                r#""name": "Tom & \"Jerry\"","#,
            ),
            (
                "manifest colour",
                &manifest,
                r##""theme_color": "#102030","##,
            ),
        ];
        for (name, file, text) in cases {
            assert!(file.contains(text), "{name}: {file}");
        }
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest).map_err(|e| CliError::Metadata(e.to_string()))?;
        assert_eq!(manifest["icons"][0]["src"], "favicon.svg", "manifest icon");
        let (index, manifest) = page(false)?;
        assert!(!index.contains("favicon"), "no icon, no favicon link");
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest).map_err(|e| CliError::Metadata(e.to_string()))?;
        assert_eq!(
            manifest["icons"],
            serde_json::json!([]),
            "no icon, no manifest icons"
        );
        Ok(())
    }

    #[test]
    fn bindgen_version_table() {
        let metadata = |versions: &[&str]| -> Metadata {
            let packages: Vec<serde_json::Value> = versions
                .iter()
                .map(|v| serde_json::json!({ "name": "wasm-bindgen", "version": v, "manifest_path": "/m", "targets": [] }))
                .chain([serde_json::json!({ "name": "hello", "version": "0.1.0", "manifest_path": "/h", "targets": [] })])
                .collect();
            serde_json::from_value(
                serde_json::json!({ "packages": packages, "target_directory": "/t" }),
            )
            .expect("test metadata")
        };
        let cases: [(&str, &[&str], Result<&str, &str>); 3] = [
            ("one version", &["0.2.130"], Ok("0.2.130")),
            ("no wasm-bindgen", &[], Err("NoWasmBindgen")),
            (
                "two versions",
                &["0.2.120", "0.2.130"],
                Err("SeveralWasmBindgens"),
            ),
        ];
        for (name, versions, want) in cases {
            let got = bindgen_version(&metadata(versions), "hello").map_err(|e| e.variant());
            assert_eq!(got, want.map(String::from).map_err(String::from), "{name}");
        }
    }

    #[test]
    fn cli_version_table() {
        let cases = [
            ("release", "wasm-bindgen 0.2.130\n", Some("0.2.130")),
            (
                "with a commit",
                "wasm-bindgen 0.2.130 (abc123)\n",
                Some("0.2.130"),
            ),
            ("another tool", "wasm-pack 0.13.1\n", None),
            ("nothing", "", None),
        ];
        for (name, stdout, want) in cases {
            assert_eq!(cli_version(stdout).as_deref(), want, "{name}");
        }
    }

    #[test]
    fn wasm_bindgen_error_names_the_fix() {
        // (row name, installed version, expected text)
        let cases = [
            (
                "missing",
                None,
                "cargo install wasm-bindgen-cli --version 0.2.130",
            ),
            (
                "mismatch",
                Some("0.2.120"),
                "cargo install wasm-bindgen-cli --version 0.2.130",
            ),
            (
                "mismatch names the installed one",
                Some("0.2.120"),
                "0.2.120",
            ),
        ];
        for (name, have, text) in cases {
            let error = CliError::WasmBindgen {
                have: have.map(String::from),
                want: "0.2.130".into(),
            };
            assert!(error.to_string().contains(text), "{name}: {error}");
        }
    }
}
