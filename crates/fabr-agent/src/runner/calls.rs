use crate::model::{CallObserver, ModelCall};
use crate::now_utc;
use fabr_core::redact::redact;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Clone, Default)]
pub struct Sink {
    captured: Option<Arc<Mutex<Vec<String>>>>,
}

impl Sink {
    pub fn capture() -> (Sink, Arc<Mutex<Vec<String>>>) {
        let lines = Arc::new(Mutex::new(Vec::new()));
        (Sink { captured: Some(lines.clone()) }, lines)
    }

    fn keep(&self, line: &str) {
        if let Some(lines) = &self.captured {
            lines.lock().unwrap_or_else(PoisonError::into_inner).push(line.to_string());
        }
    }

    pub fn info(&self, msg: impl AsRef<str>) {
        let line = redact(msg.as_ref());
        log::info!("{line}");
        self.keep(&line);
    }

    pub fn warn(&self, msg: impl AsRef<str>) {
        let line = redact(msg.as_ref());
        log::warn!("{line}");
        self.keep(&line);
    }
}

pub struct CallLog {
    sink: Sink,
    fingerprint: String,
    state: Mutex<(String, f64)>,
    /// Whether the model-reachable signal has been emitted yet.
    ///
    /// The `_backend.yaml` heartbeat only proves a hosting process is alive; it
    /// does not prove the configured model can actually be reached. The first
    /// successful `ModelCall` observed after start is the first proof that the
    /// key works and the model responds, so we emit one distinct "model
    /// reachable" INFO line through the `Sink` exactly once. A model *error*
    /// never reaches the observer — `AnthropicModel::post` returns before
    /// invoking it — so the first-error path is surfaced separately via the
    /// classified failure in `Runner::on_drive_error` (task 2.1).
    reachable_seen: AtomicBool,
}

impl CallLog {
    pub fn new(sink: Sink, fingerprint: String) -> Arc<CallLog> {
        Arc::new(CallLog { sink, fingerprint, state: Mutex::new((String::new(), 0.0)), reachable_seen: AtomicBool::new(false) })
    }

    pub fn begin(&self, record_id: &str) {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner) = (record_id.to_string(), 0.0);
    }

    pub fn take_cost(&self) -> f64 {
        std::mem::take(&mut self.state.lock().unwrap_or_else(PoisonError::into_inner).1)
    }

    pub fn observer(self: &Arc<Self>) -> CallObserver {
        let this = self.clone();
        Arc::new(move |call| this.record(call))
    }

    fn record(&self, call: &ModelCall) {
        // The observer only fires on a successful `post`, so the first call we
        // see is the first proof the model is reachable. Emit the readiness
        // signal through the Sink exactly once, before the per-call line.
        if !self.reachable_seen.swap(true, Ordering::SeqCst) {
            self.sink.info(format!("model reachable: model={} key={}", call.model, self.fingerprint));
        }
        let cost = match (call.input_tokens, call.output_tokens) {
            (Some(i), Some(o)) => Some(cost_usd(&call.model, i, o)),
            _ => None,
        };
        let id = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.1 += cost.unwrap_or(0.0);
            state.0.clone()
        };
        let n = |v: Option<u64>| v.map_or("-".to_string(), |v| v.to_string());
        let cost_text = cost.map(|c| format!(" cost_usd={c:.6}")).unwrap_or_default();
        self.sink.info(format!(
            "model call at={} record={id} step={} model={} in={} out={}{cost_text} key={}",
            now_utc(),
            call.step,
            call.model,
            n(call.input_tokens),
            n(call.output_tokens),
            self.fingerprint
        ));
    }
}

pub fn cost_usd(model: &str, input: u64, output: u64) -> f64 {
    let (pin, pout) = if model.contains("haiku") { (0.10, 0.50) } else { (2.0, 10.0) };
    (input as f64 * pin + output as f64 * pout) / 1_000_000.0
}
