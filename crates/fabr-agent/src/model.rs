use fabr_core::setup::Capabilities;
use fabr_core::{FabrError, Secret};
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftAssertion {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DraftCheck {
    pub cmd: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefineDraft {
    pub spec: String,
    pub success: String,
    pub assertions: Vec<DraftAssertion>,
    pub question: Option<String>,
    pub grants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDraft {
    pub spec: String,
    pub success: String,
    pub plan: String,
    pub assertions: Vec<DraftAssertion>,
    pub reversibility: String,
    pub tools: Vec<String>,
    pub checks: Vec<DraftCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
    pub path: String,
    pub content: Option<String>,
}

pub struct PlanInput<'a> {
    pub title: &'a str,
    pub spec: &'a str,
    pub success: &'a str,
    pub refusal_note: Option<&'a str>,
    pub answers: &'a str,
    pub allowed: &'a Capabilities,
    pub rejection: Option<&'a str>,
}

pub trait Model {
    fn define(&self, title: &str, context: &str) -> Result<DefineDraft, FabrError>;
    fn plan(&self, input: &PlanInput) -> Result<PlanDraft, FabrError>;
    fn execute(&self, title: &str, spec: &str, plan: &str) -> Result<Vec<ToolCall>, FabrError>;
}

#[derive(Debug)]
pub struct ScriptedModel {
    plans: Mutex<VecDeque<PlanDraft>>,
    calls: Mutex<Vec<ToolCall>>,
    executed: AtomicUsize,
    question: Mutex<Option<String>>,
    grants: Mutex<Vec<String>>,
    rejections: Mutex<Vec<Option<String>>>,
}

impl ScriptedModel {
    pub fn new(plan: PlanDraft, calls: Vec<ToolCall>) -> Self {
        Self {
            plans: Mutex::new(VecDeque::from([plan])),
            calls: Mutex::new(calls),
            executed: AtomicUsize::new(0),
            question: Mutex::new(None),
            grants: Mutex::new(Vec::new()),
            rejections: Mutex::new(Vec::new()),
        }
    }

    pub fn plan_rejections(&self) -> Vec<Option<String>> {
        self.rejections.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn with_grants(self, grants: &[&str]) -> Self {
        *self.grants.lock().unwrap_or_else(|p| p.into_inner()) = grants.iter().map(|g| (*g).to_string()).collect();
        self
    }

    pub fn with_question(plan: PlanDraft, question: impl Into<String>) -> Self {
        let model = Self::new(plan, Vec::new());
        *model.question.lock().unwrap_or_else(|p| p.into_inner()) = Some(question.into());
        model
    }

    pub fn push_plan(&self, plan: PlanDraft) {
        self.plans.lock().unwrap_or_else(|p| p.into_inner()).push_back(plan);
    }

    pub fn execute_count(&self) -> usize {
        self.executed.load(Ordering::SeqCst)
    }
}

impl Model for ScriptedModel {
    fn define(&self, _: &str, _: &str) -> Result<DefineDraft, FabrError> {
        let q = self.plans.lock().unwrap_or_else(|p| p.into_inner());
        let plan = q.front().cloned().ok_or_else(|| FabrError::BadRequest("no scripted plan".into()))?;
        let question = self.question.lock().unwrap_or_else(|p| p.into_inner()).take();
        Ok(DefineDraft {
            spec: plan.spec,
            success: plan.success,
            assertions: plan.assertions,
            question,
            grants: self.grants.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        })
    }

    fn plan(&self, input: &PlanInput) -> Result<PlanDraft, FabrError> {
        self.rejections.lock().unwrap_or_else(|p| p.into_inner()).push(input.rejection.map(str::to_string));
        let mut q = self.plans.lock().unwrap_or_else(|p| p.into_inner());
        q.pop_front().ok_or_else(|| FabrError::BadRequest("no scripted plan".into()))
    }

    fn execute(&self, _: &str, _: &str, _: &str) -> Result<Vec<ToolCall>, FabrError> {
        self.executed.fetch_add(1, Ordering::SeqCst);
        Ok(self.calls.lock().unwrap_or_else(|p| p.into_inner()).clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelCall {
    pub step: &'static str,
    pub model: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

pub type CallObserver = std::sync::Arc<dyn Fn(&ModelCall) + Send + Sync>;

pub struct AnthropicModel {
    api_key: Secret,
    model: String,
    base_url: String,
    observer: Option<CallObserver>,
    http: ureq::Agent,
    retry_base: std::time::Duration,
}

pub const MAX_TOKENS: u32 = 4096;
const TRIES: u32 = 3;
const RETRY_CAP: std::time::Duration = std::time::Duration::from_secs(8);
const RETRY_AFTER_CAP: std::time::Duration = std::time::Duration::from_secs(30);

impl std::fmt::Debug for AnthropicModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicModel").field("model", &self.model).finish_non_exhaustive()
    }
}

impl AnthropicModel {
    pub fn new(api_key: Secret, model: impl Into<String>) -> Self {
        Self {
            api_key,
            model: model.into(),
            base_url: "https://api.anthropic.com".into(),
            observer: None,
            http: ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(60)).build(),
            retry_base: std::time::Duration::from_secs(1),
        }
    }

    pub fn with_retry_base(mut self, base: std::time::Duration) -> Self {
        self.retry_base = base;
        self
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.trim_end_matches('/').to_string();
        self
    }

    pub fn with_observer(mut self, observer: CallObserver) -> Self {
        self.observer = Some(observer);
        self
    }

    fn send_once(&self, body: &serde_json::Value) -> Result<String, (FabrError, bool, Option<std::time::Duration>)> {
        let sent = self
            .http
            .post(&format!("{}/v1/messages", self.base_url))
            .set("x-api-key", self.api_key.expose())
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_json(body);
        match sent {
            Ok(r) => r.into_string().map_err(|e| (FabrError::Remote(e.to_string()), true, None)),
            Err(ureq::Error::Status(code, r)) => {
                let wait = r.header("retry-after").and_then(|v| v.trim().parse::<u64>().ok()).map(std::time::Duration::from_secs);
                let text = r.into_string().map_err(|e| (FabrError::Remote(e.to_string()), false, None))?;
                let retry = code == 429 || code >= 500;
                Err((FabrError::Remote(format!("anthropic {code}: {text}")), retry, wait))
            }
            Err(e) => Err((FabrError::Remote(e.to_string()), true, None)),
        }
    }

    fn delay(&self, attempt: u32, hint: Option<std::time::Duration>) -> std::time::Duration {
        let backoff = self.retry_base.saturating_mul(1 << (attempt - 1)).min(RETRY_CAP);
        hint.map_or(backoff, |h| h.min(RETRY_AFTER_CAP))
    }

    fn post(&self, step: &'static str, body: serde_json::Value) -> Result<serde_json::Value, FabrError> {
        let mut attempt = 1;
        let text = loop {
            match self.send_once(&body) {
                Ok(text) => break text,
                Err((err, retry, hint)) if retry && attempt < TRIES => {
                    log::warn!("anthropic {step} attempt {attempt} failed: {}; retrying", fabr_core::redact::redact(&err.message()));
                    std::thread::sleep(self.delay(attempt, hint));
                    attempt += 1;
                }
                Err((err, _, _)) => return Err(err),
            }
        };
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| FabrError::Remote(format!("anthropic response: {e}")))?;
        if let Some(observer) = &self.observer {
            let usage = |k: &str| value.get("usage").and_then(|u| u.get(k)).and_then(|n| n.as_u64());
            observer(&ModelCall {
                step,
                model: self.model.clone(),
                input_tokens: usage("input_tokens"),
                output_tokens: usage("output_tokens"),
            });
        }
        if value.get("stop_reason").and_then(|r| r.as_str()) == Some("max_tokens") {
            return Err(FabrError::Remote(format!("anthropic {step} response was cut off at max_tokens ({MAX_TOKENS})")));
        }
        Ok(value)
    }
}

impl Model for AnthropicModel {
    fn define(&self, title: &str, context: &str) -> Result<DefineDraft, FabrError> {
        let prompt = format!(
            "Task title: {title}\nContext:\n{context}\n\
             Return one JSON object with keys spec, success, assertions, question, grants. \
             assertions is a list of objects with id and text. \
             question is a string when something required is missing, otherwise null. \
             grants is a list of personal data field names the task needs filled in, or an empty list. \
             spec and success are non-empty strings when question is null."
        );
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "system": "Reply with only a JSON object. No markdown.",
            "messages": [{"role": "user", "content": prompt}]
        });
        let value = self.post("define", body)?;
        let text = text_of(&value)?;
        parse_define(&text)
    }

    fn plan(&self, input: &PlanInput) -> Result<PlanDraft, FabrError> {
        let PlanInput { title, spec, success, refusal_note, answers, allowed, rejection } = input;
        let note = refusal_note.unwrap_or("");
        let rejected = rejection.map(|r| format!("\nYour previous plan was rejected: {r}. Return a corrected plan.")).unwrap_or_default();
        let prompt = format!(
            "Task title: {title}\nSpec:\n{spec}\nSuccess:\n{success}\nRefusal note:\n{note}\nUser answers:{answers}\n\
             Return one JSON object with keys spec, success, plan, assertions, plan_meta. \
             assertions is a list of objects with id and text. \
             plan_meta has reversibility (reversible or irreversible), tools, \
             and checks (list of objects with cmd and args). checks is argv and must not use a shell. \
             spec, success, and plan are non-empty strings. \
             Allowed tools: {}. Allowed check commands: {}. Plan only within them; every check cmd must be one of these.{rejected}",
            allowed.tools.join(", "),
            allowed.checks.join(", ")
        );
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "system": "Reply with only a JSON object. No markdown.",
            "messages": [{"role": "user", "content": prompt}]
        });
        let value = self.post("plan", body)?;
        let text = text_of(&value)?;
        parse_plan(&text)
    }

    fn execute(&self, title: &str, spec: &str, plan: &str) -> Result<Vec<ToolCall>, FabrError> {
        let prompt = format!("Title: {title}\nSpec:\n{spec}\nPlan:\n{plan}\nUse tools to apply the plan. Stay inside the node.");
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "tools": [tool_schema("read_file", "Read a file inside the node", false), tool_schema("write_file", "Write a file inside the node", true), tool_schema("list_dir", "List a directory inside the node", false)],
            "messages": [{"role": "user", "content": prompt}]
        });
        let value = self.post("execute", body)?;
        tool_calls_of(&value)
    }
}

fn tool_schema(name: &str, description: &str, with_content: bool) -> serde_json::Value {
    let mut properties = serde_json::json!({ "path": {"type": "string"} });
    let mut required = vec!["path"];
    if with_content {
        properties["content"] = serde_json::json!({"type": "string"});
        required.push("content");
    }
    serde_json::json!({
        "name": name,
        "description": description,
        "input_schema": {"type": "object", "properties": properties, "required": required}
    })
}

fn text_of(value: &serde_json::Value) -> Result<String, FabrError> {
    let mut out = String::new();
    for block in
        value.get("content").and_then(|c| c.as_array()).ok_or_else(|| FabrError::Remote("anthropic response has no content".into()))?
    {
        if block.get("type").and_then(|t| t.as_str()) == Some("text")
            && let Some(text) = block.get("text").and_then(|t| t.as_str())
        {
            out.push_str(text);
        }
    }
    if out.is_empty() { Err(FabrError::Remote("anthropic returned no text".into())) } else { Ok(out) }
}

fn tool_calls_of(value: &serde_json::Value) -> Result<Vec<ToolCall>, FabrError> {
    let mut calls = Vec::new();
    let blocks =
        value.get("content").and_then(|c| c.as_array()).ok_or_else(|| FabrError::Remote("anthropic response has no content".into()))?;
    for block in blocks {
        if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
            continue;
        }
        let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        let input = block.get("input").cloned().unwrap_or(serde_json::Value::Null);
        let path = input.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string();
        let content = input.get("content").and_then(|c| c.as_str()).map(str::to_string);
        calls.push(ToolCall { name, path, content });
    }
    Ok(calls)
}

#[derive(Deserialize)]
struct RawPlan {
    spec: String,
    success: String,
    plan: String,
    #[serde(default)]
    assertions: Vec<RawAssertion>,
    #[serde(default)]
    plan_meta: Option<RawMeta>,
}

#[derive(Deserialize)]
struct RawAssertion {
    id: String,
    text: String,
}

#[derive(Deserialize)]
struct RawMeta {
    #[serde(default)]
    reversibility: Option<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    checks: Vec<DraftCheck>,
}

#[derive(Deserialize)]
struct RawDefine {
    #[serde(default)]
    spec: String,
    #[serde(default)]
    success: String,
    #[serde(default)]
    assertions: Vec<RawAssertion>,
    #[serde(default)]
    question: Option<String>,
    #[serde(default)]
    grants: Vec<String>,
}

fn parse_define(text: &str) -> Result<DefineDraft, FabrError> {
    let raw: RawDefine = serde_json::from_str(&json_slice(text)).map_err(|e| FabrError::Remote(format!("define was not json: {e}")))?;
    let question = raw.question.filter(|q| !q.trim().is_empty());
    Ok(DefineDraft {
        spec: raw.spec,
        success: raw.success,
        assertions: raw.assertions.into_iter().map(|a| DraftAssertion { id: a.id, text: a.text }).collect(),
        question,
        grants: raw.grants.into_iter().map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
    })
}

fn json_slice(text: &str) -> String {
    let trimmed = text.trim();
    let trimmed = trimmed.strip_prefix("```json").or_else(|| trimmed.strip_prefix("```")).unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix("```").unwrap_or(trimmed).trim();
    let start = trimmed.find('{').unwrap_or(0);
    let end = trimmed.rfind('}').map(|i| i + 1).unwrap_or(trimmed.len());
    trimmed[start..end].to_string()
}

fn parse_plan(text: &str) -> Result<PlanDraft, FabrError> {
    let raw: RawPlan = serde_json::from_str(&json_slice(text)).map_err(|e| FabrError::Remote(format!("plan was not json: {e}")))?;
    let meta = raw.plan_meta.unwrap_or(RawMeta { reversibility: None, tools: Vec::new(), checks: Vec::new() });
    Ok(PlanDraft {
        spec: raw.spec,
        success: raw.success,
        plan: raw.plan,
        assertions: raw.assertions.into_iter().map(|a| DraftAssertion { id: a.id, text: a.text }).collect(),
        reversibility: meta.reversibility.unwrap_or_else(|| "reversible".into()),
        tools: meta.tools,
        checks: meta.checks,
    })
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn define_carries_grant_needs() {
        let d = parse_define(
            r#"{"spec":"s","success":"c","assertions":[{"id":"a1","text":"t"}],"question":null,"grants":["address.zip"," ","name.first"]}"#,
        )
        .unwrap();
        assert_eq!(d.grants, vec!["address.zip".to_string(), "name.first".to_string()]);
        assert!(parse_define(r#"{"spec":"s"}"#).unwrap().grants.is_empty());
    }

    #[test]
    fn backoff_doubles_and_is_capped() {
        let m = AnthropicModel::new(Secret::new("k"), "m");
        assert_eq!(m.delay(1, None), std::time::Duration::from_secs(1));
        assert_eq!(m.delay(2, None), std::time::Duration::from_secs(2));
        assert_eq!(m.delay(9, None), RETRY_CAP);
        assert_eq!(m.delay(1, Some(std::time::Duration::from_secs(500))), RETRY_AFTER_CAP);
    }
}

#[cfg(test)]
mod live {
    use super::*;

    #[test]
    #[ignore = "reads ANTHROPIC_API_KEY from the environment"]
    fn live_anthropic_smoke() {
        let key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY");
        let model = std::env::var("FABR_ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-sonnet-5-5".into());
        let client = AnthropicModel::new(Secret::new(key), model);
        let defined = client.define("add greet.txt that contains hi", "").expect("live define");
        assert!(defined.question.is_some() || !defined.spec.trim().is_empty());
    }
}
