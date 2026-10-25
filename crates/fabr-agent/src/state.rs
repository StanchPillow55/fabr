use crate::now_utc;
use fabr_core::taskrec::{State, Step, TaskRecord};
use std::fmt::Write as _;

pub fn append_log(log: &mut String, step: &str, message: &str) {
    if !log.is_empty() && !log.ends_with('\n') {
        log.push('\n');
    }
    let _ = writeln!(log, "- {} {step}: {message}", now_utc());
}

pub fn state_name(state: State) -> &'static str {
    match state {
        State::Reminder => "reminder",
        State::Task => "task",
        State::Defined => "defined",
        State::InProgress => "in_progress",
        State::Succeeded => "succeeded",
        State::Failed => "failed",
    }
}

pub fn step_name(step: Step) -> &'static str {
    match step {
        Step::Gather => "gather",
        Step::Define => "define",
        Step::Plan => "plan",
        Step::Approve => "approve",
        Step::Execute => "execute",
        Step::Evaluate => "evaluate",
        Step::Learn => "learn",
        Step::Notify => "notify",
    }
}

pub fn stage_label(rec: &TaskRecord) -> &'static str {
    match rec.step {
        Some(step) => step_name(step),
        None => "task",
    }
}

pub fn pending_tail(rec: &TaskRecord) -> bool {
    matches!(rec.step, Some(Step::Learn | Step::Notify))
}

pub fn is_terminal(rec: &TaskRecord) -> bool {
    matches!(rec.state, State::Succeeded | State::Failed)
}

pub fn question_open(rec: &TaskRecord) -> bool {
    rec.question.as_ref().is_some_and(|q| q.answer.is_none())
}

pub fn approval_status(rec: &TaskRecord) -> &str {
    rec.approval.as_ref().map(|a| a.status.as_str()).unwrap_or("")
}

pub fn parked(rec: &TaskRecord) -> bool {
    if rec.waiting && question_open(rec) {
        return true;
    }
    rec.state == State::Defined && !matches!(approval_status(rec), "approved" | "refused")
}

pub fn waiting_on(rec: &TaskRecord) -> Option<&'static str> {
    if !rec.waiting {
        return None;
    }
    if question_open(rec) {
        Some("question")
    } else if rec.approval.as_ref().is_some_and(|a| a.status == "pending") {
        Some("approval")
    } else {
        None
    }
}

pub fn clean_message(message: &str, max: usize) -> String {
    let flat = fabr_core::redact::redact(message).split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max { flat } else { format!("{}...", flat.chars().take(max).collect::<String>()) }
}
