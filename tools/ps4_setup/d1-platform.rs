// SPDX-License-Identifier: GPL-2.0-or-later
//! Original platform orchestration, installed in the separate d1-pkg crate.
//! Uses curl for FTP and the crate's manifest digest; no game format is reimplemented.
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");
const EBOOT_SHA: &str = "7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954";
const EBOOT_BYTES: u64 = 29_148_264;
const UNION: &str = "/mnt/sandbox/pfsmnt/CUSA00219-app0-patch0-union";

fn default_curl() -> PathBuf {
    if cfg!(windows) { "curl.exe" } else { "curl" }.into()
}
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    /// Offline controls, including real curl transfers against loopback-only fixtures.
    Controls {
        #[arg(long, default_value_os_t = default_curl())]
        curl: PathBuf,
    },
    /// Retrieve only the missing bootstrap files from an already isolated owned console.
    /// Does not configure/prove console isolation and never installs the files.
    PullBootstrap {
        #[arg(long)]
        ip: Ipv4Addr,
        #[arg(long, default_value_t = 2121)]
        port: u16,
        #[arg(long, default_value = REPO)]
        repo: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long, default_value_os_t = default_curl())]
        curl: PathBuf,
    },
    /// Record the effective environment exported by the canonical Bash launcher.
    LaunchContract {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        game: PathBuf,
        #[arg(long)]
        patch: PathBuf,
        #[arg(long)]
        emulator: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Actual server/containment checks, called inside the canonical sealed namespace.
    VerifyServers {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        outer_netns: String,
        #[arg(long)]
        shad_pid: u32,
        #[arg(long)]
        responder_pid: u32,
    },
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn local_ip(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_loopback()
}
fn digest(path: &Path) -> Result<(u64, String)> {
    let bytes = fs::read(path)?;
    Ok((bytes.len() as u64, d1_pkg::manifest::sha256_hex(&bytes)))
}
fn matches(path: &Path, size: u64, sha: &str) -> bool {
    fs::metadata(path).is_ok_and(|m| m.len() == size)
        && digest(path).is_ok_and(|(s, h)| s == size && h == sha)
}
fn write_json_new(path: &Path, value: &Value) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
fn curl_fetch(curl: &Path, ip: Ipv4Addr, port: u16, remote: &str, partial: &Path) -> Result<()> {
    if !local_ip(ip)
        || port == 0
        || !remote.starts_with('/')
        || remote.contains(['\r', '\n', '?', '#', '%'])
        || remote.contains("..")
    {
        bail!("not an allowed local FTP source")
    }
    // Reserve our output; curl can truncate this invocation's file, never an existing one.
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(partial)?;
    let url = format!("ftp://{ip}:{port}{remote}");
    let output = Command::new(curl)
        .args([
            "-q",
            "--silent",
            "--show-error",
            "--globoff",
            "--proto",
            "=ftp",
            "--proxy",
            "",
            "--noproxy",
            "*",
            "--ftp-pasv",
            "--ftp-skip-pasv-ip",
            "--connect-timeout",
            "5",
            "--max-time",
            "120",
            "--max-filesize",
            "67108864",
            "--user",
            "anonymous:",
            "--url",
            &url,
            "--output",
        ])
        .arg(partial)
        .output()
        .context("starting curl")?;
    if !output.status.success() {
        bail!(
            "FTP transfer failed ({:?}); partial retained: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
    Ok(())
}
fn finish(
    partial: &Path,
    destination: &Path,
    expected: Option<(u64, &str)>,
) -> Result<(u64, String)> {
    let (size, sha) = digest(partial)?;
    if size == 0 || size > 67_108_864 {
        bail!("empty or oversized transfer")
    }
    if let Some((want_size, want_sha)) = expected {
        if size != want_size || sha != want_sha {
            bail!("transferred executable does not match exact 01.29")
        }
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(partial)?
        .sync_all()?;
    // An atomic hard link fails if the destination exists, on Windows and Unix.
    // Unlike Unix rename it cannot replace a file created between checks.
    fs::hard_link(partial, destination)?;
    fs::remove_file(partial)?;
    Ok((size, sha))
}
fn contract(repo: &Path, game: &Path, patch: &Path, emulator: &Path, out: &Path) -> Result<()> {
    let environment: BTreeMap<_, _> = std::env::vars()
        .filter(|(k, _)| {
            k.starts_with("D1_")
                || [
                    "OPERATOR_PAD",
                    "SDL_IGNORE_PADS",
                    "PRESS",
                    "PAD_FIFO",
                    "VK_DRIVER_FILES",
                ]
                .contains(&k.as_str())
        })
        .collect();
    let manifest = repo.join("capture-rig/manifests/full_en.bin");
    let fixture_missing: Vec<_> = ["D1_PROFILE_FILE", "D1_CHARACTER_FILE", "D1_ROSTER_DIR"]
        .into_iter()
        .filter_map(|key| {
            environment
                .get(key)
                .filter(|s| !s.is_empty())
                .filter(|s| !Path::new(s).exists())
                .map(|s| json!({"variable": key, "path": s}))
        })
        .collect();
    write_json_new(
        out,
        &json!({
            "scope": "canonical launch preparation, no emulator", "environment": environment,
            "game": game, "patch": patch, "linux_reference_emulator": emulator,
            "canonical_launcher_sha256": digest(&repo.join("capture-rig/run-boot.sh"))?.1,
            "canonical_delegate_sha256": digest(&repo.join("capture-rig/run-signon-experiment.sh"))?.1,
            "exact_executable_present": matches(game, EBOOT_BYTES, EBOOT_SHA),
            "pinned_manifest_present": matches(&manifest, d1_pkg::manifest::GOLDEN_FULL_EN.0 as u64, d1_pkg::manifest::GOLDEN_FULL_EN.1),
            "missing_configured_fixtures": fixture_missing,
            "windows_transport_validated": false, "native_boot_accepted": false
        }),
    )
}

fn output_text(program: &str, args: &[&str]) -> Result<(bool, String)> {
    let out = Command::new(program).args(args).output()?;
    Ok((out.status.success(), String::from_utf8(out.stdout)?))
}
fn verify_servers(out: &Path, outer: &str, shad: u32, responder: u32) -> Result<bool> {
    let namespace = fs::read_link("/proc/self/ns/net")?;
    let mut rows: Vec<(&str, bool)> = Vec::new();
    rows.push((
        "new network namespace differs from host",
        namespace.to_string_lossy() != outer,
    ));
    rows.push((
        "both owned servers inhabit this namespace",
        [shad, responder]
            .into_iter()
            .all(|pid| fs::read_link(format!("/proc/{pid}/ns/net")).is_ok_and(|n| n == namespace)),
    ));
    let (_, links) = output_text("ip", &["-j", "-d", "link", "show"])?;
    let links: Vec<Value> = serde_json::from_str(&links)?;
    rows.push((
        "only loopback and the dummy interface exist",
        links.len() == 2
            && links.iter().any(|v| v["ifname"] == "lo")
            && links
                .iter()
                .any(|v| v["ifname"] == "d1net" && v["linkinfo"]["info_kind"] == "dummy"),
    ));
    let (_, routes) = output_text("ip", &["-j", "route", "show", "table", "all"])?;
    let routes: Vec<Value> = serde_json::from_str(&routes)?;
    rows.push((
        "no IPv4 default route",
        routes.iter().all(|v| v["dst"] != "default"),
    ));
    let (_, routes6) = output_text("ip", &["-6", "-j", "route", "show", "table", "all"])?;
    let routes6: Vec<Value> = serde_json::from_str(&routes6)?;
    rows.push((
        "no IPv6 default route",
        routes6.iter().all(|v| v["dst"] != "default"),
    ));
    let (_, addresses) = output_text("ip", &["-j", "addr", "show", "dev", "d1net"])?;
    let addresses: Vec<Value> = serde_json::from_str(&addresses)?;
    rows.push((
        "dummy address is 10.77.0.2",
        addresses.iter().any(|v| {
            v["addr_info"]
                .as_array()
                .is_some_and(|a| a.iter().any(|a| a["local"] == "10.77.0.2"))
        }),
    ));
    let external: SocketAddr = "1.1.1.1:443".parse()?;
    rows.push((
        "external TCP fails immediately without a route",
        TcpStream::connect_timeout(&external, Duration::from_millis(500))
            .is_err_and(|e| e.raw_os_error() == Some(101)),
    ));
    let (resolved, _) = output_text("timeout", &["3", "getent", "ahostsv4", "one.one.one.one"])?;
    rows.push(("upstream DNS fails closed", !resolved));
    for name in [
        "signon.deadorbit.net",
        "auth.api.np.ac.playstation.net",
        "stun1.signon.deadorbit.net",
        "cdn.deadorbit.net",
    ] {
        let (ok, text) = output_text("getent", &["ahostsv4", name])?;
        rows.push((
            name,
            ok && text.split_whitespace().next() == Some("10.77.0.2"),
        ));
    }
    for (name, address) in [
        ("shadNet TCP listener", "127.0.0.1:31313"),
        ("BAP TCP listener", "10.77.0.2:3075"),
    ] {
        let address: SocketAddr = address.parse()?;
        let start = Instant::now();
        let ready = loop {
            if TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok() {
                break true;
            }
            if start.elapsed() > Duration::from_secs(8) {
                break false;
            }
            thread::sleep(Duration::from_millis(100));
        };
        rows.push((name, ready));
    }
    let request = |url: &str, ca: bool| -> Result<std::process::Output> {
        let mut cmd = Command::new("curl");
        cmd.args([
            "-q",
            "--silent",
            "--show-error",
            "--proxy",
            "",
            "--noproxy",
            "*",
            "--proto",
            "=http,https",
            "--connect-timeout",
            "2",
            "--max-time",
            "5",
        ]);
        if ca {
            cmd.arg("--cacert").arg(out.join("tls.crt"));
        }
        Ok(cmd.arg("--url").arg(url).output()?)
    };
    let golden = d1_pkg::manifest::GOLDEN_FULL_EN;
    for (label, url, ca) in [
        (
            "HTTP serves exact pinned manifest",
            "http://10.77.0.2/manifest.bin",
            false,
        ),
        (
            "WebAPI port serves exact pinned manifest",
            "http://10.77.0.2:31315/manifest.bin",
            false,
        ),
        (
            "TLS with explicit lab CA serves exact pinned manifest",
            "https://signon.deadorbit.net/manifest.bin",
            true,
        ),
    ] {
        let response = request(url, ca)?;
        rows.push((
            label,
            response.status.success()
                && response.stdout.len() == golden.0
                && d1_pkg::manifest::sha256_hex(&response.stdout) == golden.1,
        ));
    }
    let untrusted = request("https://signon.deadorbit.net/manifest.bin", false)?;
    rows.push((
        "untrusted self-signed TLS certificate is rejected",
        untrusted.status.code() == Some(60),
    ));
    let (ok, code) = output_text(
        "curl",
        &[
            "-q",
            "--silent",
            "--proxy",
            "",
            "--noproxy",
            "*",
            "--max-time",
            "3",
            "--output",
            "/dev/null",
            "--write-out",
            "%{http_code}",
            "http://10.77.0.2/unsupported-platform-control",
        ],
    )?;
    rows.push(("unsupported route returns 404", ok && code == "404"));
    let pass = rows.iter().all(|(_, pass)| *pass);
    write_json_new(
        &out.join("server-checks.json"),
        &json!({
            "scope": "actual Linux namespace and server checks, no game client", "passed": pass,
            "checks": rows, "namespace": namespace, "outer_namespace": outer,
            "owned_pids": [shad, responder], "native_boot_accepted": false, "windows_transport_validated": false
        }),
    )?;
    println!(
        "Server-only checks: {} passing of {}",
        rows.iter().filter(|(_, p)| *p).count(),
        rows.len()
    );
    Ok(pass)
}

/// Bounded synthetic FTP fixture, only for testing curl orchestration.
/// Advertises a public PASV IP to prove the real client ignores that address.
fn ftp_fixture(
    body: Vec<u8>,
    truncated: bool,
) -> Result<(SocketAddr, thread::JoinHandle<Result<()>>)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let handle = thread::spawn(move || -> Result<()> {
        let start = Instant::now();
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if start.elapsed() > Duration::from_secs(8) {
                        bail!("fixture accept timed out")
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(e.into()),
            }
        };
        // Windows accepts inherit the listener's nonblocking state.
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(8)))?;
        stream.set_write_timeout(Some(Duration::from_secs(8)))?;
        let mut reader = BufReader::new(stream.try_clone()?);
        write!(stream, "220 synthetic loopback fixture\r\n")?;
        let mut data = None;
        for _ in 0..32 {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let verb = line.split_whitespace().next().unwrap_or("");
            match verb {
                "USER" => write!(stream, "331 anonymous\r\n")?,
                "PASS" => write!(stream, "230 accepted\r\n")?,
                "PWD" => write!(stream, "257 \"/\"\r\n")?,
                "CWD" | "TYPE" => write!(stream, "200 ok\r\n")?,
                "EPSV" => write!(stream, "500 use PASV\r\n")?,
                "PASV" => {
                    let passive = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
                    passive.set_nonblocking(true)?;
                    let port = passive.local_addr()?.port();
                    write!(
                        stream,
                        "227 Entering Passive Mode (203,0,113,7,{},{})\r\n",
                        port / 256,
                        port % 256
                    )?;
                    data = Some(passive);
                }
                "SIZE" => write!(stream, "213 {}\r\n", body.len())?,
                "RETR" => {
                    write!(stream, "150 opening\r\n")?;
                    let passive = data.take().context("no passive listener")?;
                    let started = Instant::now();
                    let mut data_stream = loop {
                        match passive.accept() {
                            Ok((stream, _)) => break stream,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                if started.elapsed() > Duration::from_secs(8) {
                                    bail!("fixture data timed out")
                                }
                                thread::sleep(Duration::from_millis(10));
                            }
                            Err(e) => return Err(e.into()),
                        }
                    };
                    data_stream.set_nonblocking(false)?;
                    data_stream.set_write_timeout(Some(Duration::from_secs(8)))?;
                    let length = if truncated {
                        body.len() / 2
                    } else {
                        body.len()
                    };
                    data_stream.write_all(&body[..length])?;
                    drop(data_stream);
                    write!(
                        stream,
                        "{}\r\n",
                        if truncated {
                            "426 deliberately interrupted"
                        } else {
                            "226 complete"
                        }
                    )?;
                    if truncated {
                        break;
                    }
                }
                "QUIT" => {
                    write!(stream, "221 bye\r\n")?;
                    break;
                }
                _ => write!(stream, "500 unsupported fixture command\r\n")?,
            }
        }
        Ok(())
    });
    Ok((address, handle))
}
fn controls(curl: &Path) -> Result<Vec<(String, bool)>> {
    let root = std::env::temp_dir().join(format!(
        "d1-platform-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&root)?;
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            for name in [
                "ok.partial",
                "ok.bin",
                "bad.partial",
                "bad.bin",
                "existing.partial",
                "existing.bin",
            ] {
                let _ = fs::remove_file(self.0.join(name));
            }
            let _ = fs::remove_dir(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let mut rows = vec![
        (
            "public and unspecified source IPs rejected".into(),
            !local_ip(Ipv4Addr::new(1, 1, 1, 1)) && !local_ip(Ipv4Addr::UNSPECIFIED),
        ),
        (
            "literal isolated-LAN and loopback IPs accepted".into(),
            local_ip(Ipv4Addr::new(10, 99, 99, 20)) && local_ip(Ipv4Addr::LOCALHOST),
        ),
        (
            "unsafe run names rejected".into(),
            !safe_name("../dump") && !safe_name("name;cmd") && safe_name("owned-01_29"),
        ),
        (
            "target executable identity remains pinned".into(),
            EBOOT_BYTES == 29_148_264 && EBOOT_SHA.len() == 64,
        ),
    ];
    let body = b"synthetic FTP payload - no game bytes".to_vec();
    let sha = d1_pkg::manifest::sha256_hex(&body);
    let (address, handle) = ftp_fixture(body.clone(), false)?;
    let partial = root.join("ok.partial");
    let result = curl_fetch(
        curl,
        Ipv4Addr::LOCALHOST,
        address.port(),
        "/fixture.bin",
        &partial,
    );
    let server = handle
        .join()
        .map_err(|_| anyhow::anyhow!("fixture panicked"))?;
    if let Err(e) = &result {
        eprintln!("Positive FTP control: {e:#}");
    }
    if let Err(e) = &server {
        eprintln!("Positive FTP fixture: {e:#}");
    }
    rows.push((
        "real curl transfer ignores public PASV address and matches fixture digest".into(),
        result.is_ok() && server.is_ok() && matches(&partial, body.len() as u64, &sha),
    ));
    rows.push((
        "wrong-build digest rejected before promotion".into(),
        finish(
            &partial,
            &root.join("bad.bin"),
            Some((EBOOT_BYTES, EBOOT_SHA)),
        )
        .is_err()
            && !root.join("bad.bin").exists(),
    ));
    rows.push((
        "validated transfer promoted with no partial remaining".into(),
        finish(
            &partial,
            &root.join("ok.bin"),
            Some((body.len() as u64, &sha)),
        )
        .is_ok()
            && !partial.exists(),
    ));
    let (address, handle) = ftp_fixture(body.clone(), true)?;
    let partial = root.join("bad.partial");
    let result = curl_fetch(
        curl,
        Ipv4Addr::LOCALHOST,
        address.port(),
        "/fixture.bin",
        &partial,
    );
    let server = handle
        .join()
        .map_err(|_| anyhow::anyhow!("fixture panicked"))?;
    rows.push((
        "interrupted FTP fails and partial is retained".into(),
        result.is_err()
            && server.is_ok()
            && matches(
                &partial,
                (body.len() / 2) as u64,
                &d1_pkg::manifest::sha256_hex(&body[..body.len() / 2]),
            )
            && !root.join("bad.bin").exists(),
    ));
    let partial = root.join("existing.partial");
    fs::write(&partial, &body)?;
    let existing = root.join("existing.bin");
    fs::write(&existing, b"preserve")?;
    rows.push((
        "existing destination cannot be overwritten".into(),
        finish(&partial, &existing, None).is_err() && fs::read(&existing)? == b"preserve",
    ));
    rows.push((
        "existing partial refused before any network request".into(),
        curl_fetch(curl, Ipv4Addr::LOCALHOST, 1, "/fixture.bin", &partial).is_err(),
    ));
    Ok(rows)
}
fn main() -> Result<ExitCode> {
    match Cli::parse().cmd {
        Cmd::VerifyServers {
            out,
            outer_netns,
            shad_pid,
            responder_pid,
        } => Ok(
            if verify_servers(&out, &outer_netns, shad_pid, responder_pid)? {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            },
        ),
        Cmd::Controls { curl } => {
            let rows = controls(&curl)
                .unwrap_or_else(|e| vec![(format!("controls runnable: {e}"), false)]);
            let pass = rows.iter().all(|(_, p)| *p);
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &json!({"scope": "offline platform controls, synthetic loopback FTP", "passed": pass, "controls": rows})
                )?
            );
            Ok(if pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            })
        }
        Cmd::LaunchContract {
            repo,
            game,
            patch,
            emulator,
            out,
        } => {
            contract(&repo, &game, &patch, &emulator, &out)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::PullBootstrap {
            ip,
            port,
            repo,
            name,
            curl,
        } => {
            if !local_ip(ip) || port == 0 || !safe_name(&name) {
                bail!("invalid local source or staging name")
            }
            let rows = controls(&curl)?;
            if rows.iter().any(|(_, pass)| !pass) {
                return Ok(ExitCode::from(2));
            }
            let root = repo
                .canonicalize()?
                .join("cfw-staging")
                .join(format!("owned-bootstrap-{name}"));
            fs::create_dir(&root)
                .context("refusing an existing or unavailable staging directory")?;
            let mut files = Vec::new();
            for (remote, name, expected) in [
                (
                    format!("{UNION}/eboot.bin"),
                    "eboot.bin",
                    Some((EBOOT_BYTES, EBOOT_SHA)),
                ),
                (
                    "/system_data/priv/appmeta/CUSA00219/param.sfo".into(),
                    "param.sfo",
                    None,
                ),
            ] {
                let partial = root.join(format!("{name}.partial"));
                curl_fetch(&curl, ip, port, &remote, &partial)?;
                let (bytes, sha) = finish(&partial, &root.join(name), expected)?;
                files.push(json!({"name": name, "source": remote, "bytes": bytes, "sha256": sha, "exact_identity_checked": expected.is_some()}));
            }
            write_json_new(
                &root.join("transfer.json"),
                &json!({
                    "scope": "owned console bootstrap only, not installed", "files": files,
                    "console_ip": ip, "console_port": port, "exact_executable_verified": true,
                    "sfo_version_verified": false, "console_isolation_proven_by_tool": false,
                    "native_boot_accepted": false, "controls_passed": rows.len()
                }),
            )?;
            println!("Verified executable staged privately at {}. Metadata still needs version review; nothing installed.", root.display());
            Ok(ExitCode::SUCCESS)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_platform_controls_pass_with_real_curl() {
        let rows = controls(&default_curl()).unwrap();
        assert_eq!(rows.len(), 10);
        for (name, pass) in rows {
            assert!(pass, "{name}");
        }
    }
    #[test]
    fn ftp_path_escape_is_rejected_without_output() {
        let path = std::env::temp_dir().join("d1-forbidden-transfer-output");
        assert!(curl_fetch(
            &default_curl(),
            Ipv4Addr::LOCALHOST,
            2121,
            "/../eboot.bin",
            &path
        )
        .is_err());
        assert!(!path.exists());
    }
}
