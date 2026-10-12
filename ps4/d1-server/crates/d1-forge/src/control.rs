// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The control-plane client: one blocking HTTP call, on a plain thread.
//!
//! Shape mandated by `historicaldocs/design/interface-architecture.md` §5b - *"one plain thread doing blocking HTTP
//! (`ureq`), a std channel, and a `request_repaint()` poke when fresh data lands. **No tokio runtime
//! inside the GUI.**"* The repaint poke comes from the **producer** side, which is why `Context` is
//! `Send + Sync + Clone` (`historicaldocs/design/egui-practices.md`, "Repaint discipline"). The UI never polls.
//!
//! The contract itself lives in `d1_loadout::apply` - this only speaks it.
//!
//! # Why the failure taxonomy is four cases and not one
//!
//! Nothing serves `/control/loadout/apply` today. "Failed" would therefore be the *normal* answer,
//! and a UI that says only "failed" trains you to ignore it - at which point the day the route does
//! exist and returns 500, you will not notice. So the outcomes are distinguished at the point where
//! the information exists, and the UI renders each differently.

use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use d1_loadout::{ApplyRequest, ApplyResponse, APPLY_PATH};

/// What came back from an apply attempt.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// 2xx with a body we could parse.
    Answered { status: u16, response: Box<ApplyResponse> },
    /// 2xx, but the body was not an [`ApplyResponse`]. Something else is on that port.
    Unparseable { status: u16, body: String, error: String },
    /// 404 or 405 - a server answered, this route is not implemented. **The expected answer today.**
    RouteMissing { status: u16, body: String },
    /// 501, or any other non-2xx. The server exists and declined.
    Refused { status: u16, body: String },
    /// Never got an HTTP response at all: connection refused, DNS, timeout.
    Unreachable { error: String },
}

impl Outcome {
    /// One line for the status bar.
    pub fn headline(&self) -> String {
        match self {
            Outcome::Answered { status, response } => {
                if response.accepted {
                    format!("applied - HTTP {status}: {}", response.detail)
                } else {
                    format!("declined - HTTP {status}: {}", response.detail)
                }
            }
            Outcome::Unparseable { status, error, .. } => {
                format!("HTTP {status}, but the body is not an ApplyResponse: {error}")
            }
            Outcome::RouteMissing { status, .. } => format!(
                "control plane reachable, route not implemented (HTTP {status} on {APPLY_PATH})"
            ),
            Outcome::Refused { status, .. } => format!("control plane refused: HTTP {status}"),
            Outcome::Unreachable { error } => {
                format!("control plane not reachable: {error}")
            }
        }
    }

    /// True only when the server actually changed something. Drives the colour, and deliberately
    /// treats "reachable but unimplemented" as *not* success.
    pub fn is_success(&self) -> bool {
        matches!(self, Outcome::Answered { response, .. } if response.accepted)
    }

    /// True for the states that mean "expected, nothing is broken here".
    pub fn is_expected_gap(&self) -> bool {
        matches!(self, Outcome::RouteMissing { .. } | Outcome::Unreachable { .. })
    }

    pub fn detail(&self) -> String {
        match self {
            Outcome::Answered { response, .. } => {
                let mut s = String::new();
                for i in &response.issues {
                    s.push_str(&format!("{i}\n"));
                }
                s
            }
            Outcome::Unparseable { body, .. }
            | Outcome::RouteMissing { body, .. }
            | Outcome::Refused { body, .. } => body.chars().take(2000).collect(),
            Outcome::Unreachable { error } => error.clone(),
        }
    }
}

/// A request in flight, plus its result channel.
pub struct InFlight {
    pub rx: Receiver<Outcome>,
}

/// POST the request on a detached thread. Returns immediately.
///
/// `repaint` is poked once, from this thread, when the answer lands - the producer-side wake that
/// keeps the GUI's idle CPU at zero instead of spinning on `request_repaint()` every frame.
pub fn spawn_apply(base_url: String, req: ApplyRequest, repaint: egui::Context) -> InFlight {
    let (tx, rx): (Sender<Outcome>, Receiver<Outcome>) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let outcome = post(&base_url, &req);
        // Send before the repaint poke: a wake that arrives before the data would just be a wasted
        // frame, and the receiver is drained in `App::logic`.
        let _ = tx.send(outcome);
        repaint.request_repaint();
    });
    InFlight { rx }
}

fn post(base_url: &str, req: &ApplyRequest) -> Outcome {
    let url = format!("{}{}", base_url.trim_end_matches('/'), APPLY_PATH);
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        // The control plane is ours and on loopback. A redirect would mean something is very wrong,
        // and following it would send the loadout somewhere we did not choose.
        .max_redirects(0)
        .build()
        .new_agent();

    let body = match serde_json::to_string(req) {
        Ok(b) => b,
        Err(e) => return Outcome::Unreachable { error: format!("serialising the request: {e}") },
    };

    match agent
        .post(&url)
        .header("content-type", "application/json")
        .send(&body)
    {
        Ok(mut resp) => {
            let status = resp.status().as_u16();
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            if (200..300).contains(&status) {
                match serde_json::from_str::<ApplyResponse>(&text) {
                    Ok(r) => Outcome::Answered { status, response: Box::new(r) },
                    Err(e) => Outcome::Unparseable { status, body: text, error: e.to_string() },
                }
            } else if status == 404 || status == 405 {
                Outcome::RouteMissing { status, body: text }
            } else {
                Outcome::Refused { status, body: text }
            }
        }
        Err(ureq::Error::StatusCode(code)) => {
            // ureq surfaces some non-2xx as an error depending on config; map it the same way so
            // the two paths cannot disagree about what a 404 means.
            if code == 404 || code == 405 {
                Outcome::RouteMissing { status: code, body: String::new() }
            } else {
                Outcome::Refused { status: code, body: String::new() }
            }
        }
        Err(e) => Outcome::Unreachable { error: e.to_string() },
    }
}

/// A short, human-legible request id. Not a UUID - this is a correlation tag in a single-operator
/// lab, and pulling in a UUID crate to make it look official would be the wrong trade.
pub fn request_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("forge-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use d1_loadout::ValidationIssue;

    #[test]
    fn headlines_distinguish_the_four_failure_modes() {
        let cases = [
            Outcome::Unreachable { error: "connection refused".into() },
            Outcome::RouteMissing { status: 404, body: String::new() },
            Outcome::Refused { status: 501, body: String::new() },
            Outcome::Unparseable { status: 200, body: "<html>".into(), error: "expected value".into() },
        ];
        let lines: Vec<String> = cases.iter().map(|c| c.headline()).collect();
        // Every one must read differently - collapsing them is the failure this enum exists to stop.
        for i in 0..lines.len() {
            for j in (i + 1)..lines.len() {
                assert_ne!(lines[i], lines[j]);
            }
        }
        assert!(!cases.iter().any(|c| c.is_success()));
        assert!(cases[0].is_expected_gap() && cases[1].is_expected_gap());
        assert!(!cases[2].is_expected_gap(), "a 501 is a real refusal, not an expected gap");
    }

    #[test]
    fn an_accepted_response_is_the_only_success() {
        let ok = Outcome::Answered {
            status: 200,
            response: Box::new(ApplyResponse { accepted: true, detail: "done".into(), ..Default::default() }),
        };
        assert!(ok.is_success());
        let declined = Outcome::Answered {
            status: 200,
            response: Box::new(ApplyResponse {
                accepted: false,
                detail: "no such character".into(),
                issues: vec![ValidationIssue {
                    severity: d1_loadout::Severity::Error,
                    code: "no_character".into(),
                    message: "index 2 is empty".into(),
                }],
                ..Default::default()
            }),
        };
        assert!(!declined.is_success());
        assert!(declined.detail().contains("index 2 is empty"));
    }

    #[test]
    fn unreachable_when_nothing_is_listening() {
        // Port 1 on loopback: nothing binds it, so this exercises the real transport path and must
        // land in `Unreachable`, not in a status-code branch. No server is started and nothing is
        // launched - this is a connect() that fails.
        let out = post("http://127.0.0.1:1", &ApplyRequest::default());
        assert!(matches!(out, Outcome::Unreachable { .. }), "{out:?}");
    }

    #[test]
    fn request_ids_are_distinct() {
        assert_ne!(request_id(), request_id());
    }
}
