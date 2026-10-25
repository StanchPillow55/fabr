use crate::model::{CallObserver, ModelCall};
use crate::now_utc;
use fabr_core::redact::redact;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StepUsage {
    pub calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub usd: f64,
}

#[derive(Default)]
struct Totals {
    record_id: String,
    cost: f64,
    usage: HashMap<String, BTreeMap<String, StepUsage>>,
}

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
    state: Mutex<Totals>,
}

impl CallLog {
    pub fn new(sink: Sink, fingerprint: String) -> Arc<CallLog> {
        Arc::new(CallLog { sink, fingerprint, state: Mutex::new(Totals::default()) })
    }

    pub fn begin(&self, record_id: &str) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.record_id = record_id.to_string();
        state.cost = 0.0;
    }

    pub fn take_cost(&self) -> f64 {
        std::mem::take(&mut self.state.lock().unwrap_or_else(PoisonError::into_inner).cost)
    }

    pub fn pending_cost(&self) -> f64 {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).cost
    }

    pub fn usage(&self, record_id: &str) -> BTreeMap<String, StepUsage> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).usage.get(record_id).cloned().unwrap_or_default()
    }

    pub fn forget(&self, record_id: &str) {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).usage.remove(record_id);
    }

    pub fn observer(self: &Arc<Self>) -> CallObserver {
        let this = self.clone();
        Arc::new(move |call| this.record(call))
    }

    fn record(&self, call: &ModelCall) {
        let cost = match (call.input_tokens, call.output_tokens) {
            (Some(i), Some(o)) => Some(cost_usd(&call.model, i, o)),
            _ => None,
        };
        let id = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.cost += cost.unwrap_or(0.0);
            let id = state.record_id.clone();
            let slot = state.usage.entry(id.clone()).or_default().entry(call.step.to_string()).or_default();
            slot.calls += 1;
            slot.input_tokens += call.input_tokens.unwrap_or(0);
            slot.output_tokens += call.output_tokens.unwrap_or(0);
            slot.usd += cost.unwrap_or(0.0);
            id
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
