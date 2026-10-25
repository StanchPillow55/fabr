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

pub trait Model {
    fn define(&self, title: &str, context: &str) -> Result<DefineDraft, FabrError>;
    fn plan(&self, title: &str, spec: &str, success: &str, refusal_note: Option<&str>) -> Result<PlanDraft, FabrError>;
    fn execute(&self, title: &str, spec: &str, plan: &str) -> Result<Vec<ToolCall>, FabrError>;
}

#[derive(Debug)]
pub struct ScriptedModel {
    plans: Mutex<VecDeque<PlanDraft>>,
    calls: Mutex<Vec<ToolCall>>,
    executed: AtomicUsize,
    question: Mutex<Option<String>>,
}

impl ScriptedModel {
    pub fn new(plan: PlanDraft, calls: Vec<ToolCall>) -> Self {
        Self {
            plans: Mutex::new(VecDeque::from([plan])),
            calls: Mutex::new(calls),
            executed: AtomicUsize::new(0),
            question: Mutex::new(None),
        }
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
        Ok(DefineDraft { spec: plan.spec, success: plan.success, assertions: plan.assertions, question })
    }

    fn plan(&self, _: &str, _: &str, _: &str, _: Option<&str>) -> Result<PlanDraft, FabrError> {
        let mut q = self.plans.lock().unwrap_or_else(|p| p.into_inner());
        q.pop_front().ok_or_else(|| FabrError::BadRequest("no scripted plan".into()))
    }

    fn execute(&self, _: &str, _: &str, _: &str) -> Result<Vec<ToolCall>, FabrError> {
        self.executed.fetch_add(1, Ordering::SeqCst);
        Ok(self.calls.lock().unwrap_or_else(|p| p.into_inner()).clone())
    }
}

#[derive(Debug)]
pub struct AnthropicModel {
    api_key: Secret,
    model: String,
}

impl AnthropicModel {
    pub fn new(api_key: Secret, model: impl Into<String>) -> Self {
        Self { api_key, model: model.into() }
    }

    fn post(&self, body: serde_json::Value) -> Result<serde_json::Value, FabrError> {
        let sent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .post("https://api.anthropic.com/v1/messages")
            .set("x-api-key", self.api_key.expose())
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_json(&body);
        let resp = match sent {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) => {
                let text = r.into_string().map_err(|e| FabrError::Remote(e.to_string()))?;
                return Err(FabrError::Remote(format!("anthropic {code}: {text}")));
            }
            Err(e) => return Err(FabrError::Remote(e.to_string())),
        };
        let text = resp.into_string().map_err(|e| FabrError::Remote(e.to_string()))?;
        serde_json::from_str(&text).map_err(|e| FabrError::Remote(format!("anthropic response: {e}")))
    }
}

impl Model for AnthropicModel {
    fn define(&self, title: &str, context: &str) -> Result<DefineDraft, FabrError> {
        let prompt = format!(
            "Task title: {title}\nContext:\n{context}\n\
             Return one JSON object with keys spec, success, assertions, question. \
             assertions is a list of objects with id and text. \
             question is a string when something required is missing, otherwise null. \
             spec and success are non-empty strings when question is null."
        );
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "system": "Reply with only a JSON object. No markdown.",
            "messages": [{"role": "user", "content": prompt}]
        });
        let value = self.post(body)?;
        let text = text_of(&value)?;
        parse_define(&text)
    }

    fn plan(&self, title: &str, spec: &str, success: &str, refusal_note: Option<&str>) -> Result<PlanDraft, FabrError> {
        let note = refusal_note.unwrap_or("");
        let prompt = format!(
            "Task title: {title}\nSpec:\n{spec}\nSuccess:\n{success}\nRefusal note:\n{note}\n\
             Return one JSON object with keys spec, success, plan, assertions, plan_meta. \
             assertions is a list of objects with id and text. \
             plan_meta has reversibility (reversible or irreversible), tools (read_file, write_file, list_dir), \
             and checks (list of objects with cmd and args). checks is argv and must not use a shell. \
             spec, success, and plan are non-empty strings. \
             Do not request browser, payment, or network tools."
        );
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "system": "Reply with only a JSON object. No markdown.",
            "messages": [{"role": "user", "content": prompt}]
        });
        let value = self.post(body)?;
        let text = text_of(&value)?;
        parse_plan(&text)
    }

    fn execute(&self, title: &str, spec: &str, plan: &str) -> Result<Vec<ToolCall>, FabrError> {
        let prompt = format!("Title: {title}\nSpec:\n{spec}\nPlan:\n{plan}\nUse tools to apply the plan. Stay inside the node.");
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "tools": [tool_schema("read_file", "Read a file inside the node", false), tool_schema("write_file", "Write a file inside the node", true), tool_schema("list_dir", "List a directory inside the node", false)],
            "messages": [{"role": "user", "content": prompt}]
        });
        let value = self.post(body)?;
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
}

fn parse_define(text: &str) -> Result<DefineDraft, FabrError> {
    let raw: RawDefine = serde_json::from_str(&json_slice(text)).map_err(|e| FabrError::Remote(format!("define was not json: {e}")))?;
    let question = raw.question.filter(|q| !q.trim().is_empty());
    Ok(DefineDraft {
        spec: raw.spec,
        success: raw.success,
        assertions: raw.assertions.into_iter().map(|a| DraftAssertion { id: a.id, text: a.text }).collect(),
        question,
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
