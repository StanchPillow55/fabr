use fabr_core::FabrError;
use fabr_core::taskrec::Check;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutcome {
    pub pass: bool,
    pub evidence: String,
}

pub fn run_checks(node_root: &Path, checks: &[Check]) -> Result<Vec<CheckOutcome>, FabrError> {
    let mut out = Vec::with_capacity(checks.len());
    for check in checks {
        let status = spawn_allowed(node_root, checks, check);
        match status {
            Ok(status) => {
                let code = status.code().unwrap_or(-1);
                out.push(CheckOutcome { pass: status.success(), evidence: format!("{} {} exit {code}", check.cmd, check.args.join(" ")) });
            }
            Err(e) if e.message().contains("not on the list") => return Err(e),
            Err(e) => out.push(CheckOutcome {
                pass: false,
                evidence: format!("{} {} failed to start: {}", check.cmd, check.args.join(" "), e.message()),
            }),
        }
    }
    Ok(out)
}

pub fn spawn_allowed(node_root: &Path, allowed: &[Check], check: &Check) -> Result<std::process::ExitStatus, FabrError> {
    if !allowed.iter().any(|c| c.cmd == check.cmd && c.args == check.args) {
        return Err(FabrError::BadRequest(format!("command is not on the list: {} {}", check.cmd, check.args.join(" "))));
    }
    Command::new(&check.cmd)
        .args(&check.args)
        .current_dir(node_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(FabrError::from)
}
