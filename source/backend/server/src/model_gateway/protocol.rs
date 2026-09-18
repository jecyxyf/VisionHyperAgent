//! Explicit Chat Completions -> Responses compatibility, used only when configured.
//! A completed response requires BOTH a provider finish reason and its [DONE] marker.
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

pub type Result<T> = std::result::Result<T, String>;

#[path = "request.rs"]
mod request;
pub use request::convert_request;

#[derive(Default)]
struct Tool {
    id: String,
    name: String,
    arguments: String,
}
struct Segment {
    id: String,
    index: usize,
    text: String,
}
pub struct ChatStream {
    response_id: String,
    model: String,
    sequence: u64,
    next_index: usize,
    text: Option<Segment>,
    reasoning: Option<Segment>,
    tools: BTreeMap<usize, Tool>,
    custom_tools: HashSet<String>,
    tool_names: HashMap<String, (String, Option<String>)>,
    finish_reason: Option<String>,
    usage: Value,
}

impl ChatStream {
    pub fn new(model: String, custom_tools: HashSet<String>) -> Self {
        Self {
            response_id: format!("resp_vha_{}", uuid::Uuid::new_v4().simple()),
            model,
            sequence: 0,
            next_index: 0,
            text: None,
            reasoning: None,
            tools: BTreeMap::new(),
            custom_tools,
            tool_names: HashMap::new(),
            finish_reason: None,
            usage: Value::Null,
        }
    }
    pub fn with_tool_names(mut self, names: HashMap<String, (String, Option<String>)>) -> Self {
        self.tool_names = names;
        self
    }

    fn tag(&mut self, mut event: Value) -> Value {
        event["sequence_number"] = json!(self.sequence);
        self.sequence += 1;
        event
    }
    pub fn begin(&mut self) -> Vec<Value> {
        let response = json!({"id":self.response_id,"object":"response","status":"in_progress","model":self.model,"output":[]});
        vec![
            self.tag(json!({"type":"response.created","response":response})),
            self.tag(json!({"type":"response.in_progress","response":response})),
        ]
    }
    pub fn push(&mut self, chunk: &Value) -> Result<Vec<Value>> {
        if chunk.get("error").is_some() {
            return Err("upstream model returned a stream error".into());
        }
        if let Some(usage) = chunk.get("usage").filter(|v| !v.is_null()) {
            self.usage = usage.clone();
        }
        let Some(choices) = chunk.get("choices").and_then(Value::as_array) else {
            return Err("invalid Chat stream event".into());
        };
        if choices.len() > 1 {
            return Err("multiple Chat choices are not supported".into());
        }
        let Some(choice) = choices.first() else {
            return Ok(vec![]);
        };
        if choice.get("index").and_then(Value::as_u64).unwrap_or(0) != 0 {
            return Err("unexpected Chat choice index".into());
        }
        let delta = choice.get("delta").unwrap_or(&Value::Null);
        let mut events = Vec::new();
        if let Some(reasoning) = delta
            .get("reasoning_content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            events.extend(self.append(reasoning, true)?);
        }
        if let Some(text) = delta
            .get("content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            events.extend(self.append(text, false)?);
        }
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let index = call
                    .get("index")
                    .and_then(Value::as_u64)
                    .ok_or("tool fragment has no index")? as usize;
                if index >= 64 {
                    return Err("too many tool calls".into());
                }
                let tool = self.tools.entry(index).or_default();
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    tool.id.push_str(id);
                }
                if let Some(name) = call.pointer("/function/name").and_then(Value::as_str) {
                    tool.name.push_str(name);
                }
                if let Some(arguments) = call.pointer("/function/arguments").and_then(Value::as_str)
                {
                    tool.arguments.push_str(arguments);
                }
                if tool.arguments.len() > 1024 * 1024
                    || tool.name.len() > 256
                    || tool.id.len() > 256
                {
                    return Err("model tool payload exceeded limit".into());
                }
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.into());
        }
        Ok(events.into_iter().map(|event| self.tag(event)).collect())
    }

    fn append(&mut self, delta: &str, reasoning: bool) -> Result<Vec<Value>> {
        let slot = if reasoning {
            &mut self.reasoning
        } else {
            &mut self.text
        };
        let mut events = Vec::new();
        if slot.is_none() {
            let prefix = if reasoning { "reasoning" } else { "msg" };
            let id = format!("{prefix}_{}", uuid::Uuid::new_v4().simple());
            let index = self.next_index;
            self.next_index += 1;
            let item = if reasoning {
                json!({"type":"reasoning","id":id,"summary":[]})
            } else {
                json!({"type":"message","id":id,"role":"assistant","status":"in_progress","content":[]})
            };
            events.push(
                json!({"type":"response.output_item.added","output_index":index,"item":item}),
            );
            events.push(if reasoning {json!({"type":"response.reasoning_summary_part.added","item_id":id,"output_index":index,"summary_index":0,"part":{"type":"summary_text","text":""}})}
                else{json!({"type":"response.content_part.added","item_id":id,"output_index":index,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}})});
            *slot = Some(Segment {
                id,
                index,
                text: String::new(),
            });
        }
        let segment = slot.as_mut().expect("segment initialized");
        if segment.text.len() + delta.len() > 2 * 1024 * 1024 {
            return Err("model text exceeded limit".into());
        }
        segment.text.push_str(delta);
        events.push(if reasoning {json!({"type":"response.reasoning_summary_text.delta","item_id":segment.id,"output_index":segment.index,"summary_index":0,"delta":delta})}
            else{json!({"type":"response.output_text.delta","item_id":segment.id,"output_index":segment.index,"content_index":0,"delta":delta})});
        Ok(events)
    }

    /// Called only on a literal upstream [DONE], never simply because TCP reached EOF.
    pub fn finish(&mut self) -> Result<Vec<Value>> {
        let reason = self
            .finish_reason
            .as_deref()
            .ok_or("upstream [DONE] arrived without a finish reason")?;
        if !matches!(
            reason,
            "stop" | "tool_calls" | "function_call" | "length" | "content_filter"
        ) {
            return Err("unknown model finish reason".into());
        }
        let mut events = Vec::new();
        let mut output = BTreeMap::new();
        if let Some(segment) = &self.reasoning {
            let item = json!({"type":"reasoning","id":segment.id,"summary":[{"type":"summary_text","text":segment.text}]});
            events.push(json!({"type":"response.reasoning_summary_text.done","item_id":segment.id,"output_index":segment.index,"summary_index":0,"text":segment.text}));
            events.push(json!({"type":"response.output_item.done","output_index":segment.index,"item":item}));
            output.insert(segment.index, item);
        }
        if let Some(segment) = &self.text {
            let part = json!({"type":"output_text","text":segment.text,"annotations":[]});
            let item = json!({"type":"message","id":segment.id,"role":"assistant","status":"completed","content":[part]});
            events.push(json!({"type":"response.output_text.done","item_id":segment.id,"output_index":segment.index,"content_index":0,"text":segment.text}));
            events.push(json!({"type":"response.content_part.done","item_id":segment.id,"output_index":segment.index,"content_index":0,"part":part}));
            events.push(json!({"type":"response.output_item.done","output_index":segment.index,"item":item}));
            output.insert(segment.index, item);
        }
        // Tool calls are released only after terminal confirmation, so a truncated upstream
        // stream cannot accidentally execute a partial call and then get retried by Codex.
        for tool in self
            .tools
            .values()
            .filter(|_| !matches!(reason, "length" | "content_filter"))
        {
            if tool.id.is_empty() || tool.name.is_empty() {
                return Err("incomplete model tool metadata".into());
            }
            let index = self.next_index;
            self.next_index += 1;
            let custom = self.custom_tools.contains(&tool.name);
            let id = format!(
                "{}_{}",
                if custom { "ctc" } else { "fc" },
                uuid::Uuid::new_v4().simple()
            );
            let (name, namespace) = self
                .tool_names
                .get(&tool.name)
                .cloned()
                .unwrap_or_else(|| (tool.name.clone(), None));
            let argument = if custom {
                serde_json::from_str::<Value>(&tool.arguments)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("input")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| tool.arguments.clone())
            } else {
                tool.arguments.clone()
            };
            let mut item = if custom {
                json!({"type":"custom_tool_call","id":id,"call_id":tool.id,"name":name,"input":argument,"status":"completed"})
            } else {
                json!({"type":"function_call","id":id,"call_id":tool.id,"name":name,"arguments":argument,"status":"completed"})
            };
            if let Some(namespace) = namespace {
                item["namespace"] = json!(namespace);
            }
            let mut initial = item.clone();
            initial[if custom { "input" } else { "arguments" }] = json!("");
            initial["status"] = json!("in_progress");
            events.push(
                json!({"type":"response.output_item.added","output_index":index,"item":initial}),
            );
            events.push(if custom {json!({"type":"response.custom_tool_call_input.delta","item_id":id,"output_index":index,"delta":argument})}
                else{json!({"type":"response.function_call_arguments.delta","item_id":id,"output_index":index,"delta":argument})});
            events.push(if custom {json!({"type":"response.custom_tool_call_input.done","item_id":id,"output_index":index,"input":argument})}
                else{json!({"type":"response.function_call_arguments.done","item_id":id,"output_index":index,"arguments":argument})});
            events
                .push(json!({"type":"response.output_item.done","output_index":index,"item":item}));
            output.insert(index, item);
        }
        let incomplete = matches!(reason, "length" | "content_filter");
        let usage = if self.usage["prompt_tokens"].is_number()
            && self.usage["completion_tokens"].is_number()
        {
            let mut usage = json!({"input_tokens":self.usage["prompt_tokens"],"output_tokens":self.usage["completion_tokens"],"total_tokens":self.usage["total_tokens"]});
            if let Some(cached) = self.usage.pointer("/prompt_tokens_details/cached_tokens") {
                usage["input_tokens_details"] = json!({"cached_tokens":cached});
            }
            if let Some(reasoning) = self
                .usage
                .pointer("/completion_tokens_details/reasoning_tokens")
            {
                usage["output_tokens_details"] = json!({"reasoning_tokens":reasoning});
            }
            usage
        } else {
            Value::Null
        };
        let mut response = json!({"id":self.response_id,"object":"response","model":self.model,"status":if incomplete{"incomplete"}else{"completed"},"output":output.into_values().collect::<Vec<_>>(),"usage":usage});
        if incomplete {
            response["incomplete_details"] =
                json!({"reason":if reason=="length"{"max_output_tokens"}else{"content_filter"}});
        }
        events.push(json!({"type":if incomplete{"response.incomplete"}else{"response.completed"},"response":response}));
        Ok(events.into_iter().map(|event| self.tag(event)).collect())
    }

    pub fn fail(&mut self, reason: &str) -> Value {
        self.tag(json!({"type":"response.failed","response":{"id":self.response_id,"status":"failed","error":{"code":"provider_stream_error","message":reason},"output":[]}}))
    }
}

/// Incremental SSE framing. UTF-8 decoding happens only after a complete event is assembled.
#[derive(Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
    data: Vec<String>,
}
impl SseDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>> {
        self.buffer.extend_from_slice(bytes);
        if self.buffer.len() > 2 * 1024 * 1024 {
            return Err("provider SSE frame exceeded limit".into());
        }
        let mut events = Vec::new();
        while let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
            let mut line = self.buffer.drain(..=end).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8(line).map_err(|_| "invalid provider SSE encoding")?;
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(self.data.join("\n"));
                    self.data.clear();
                }
            } else if let Some(value) = line.strip_prefix("data:") {
                self.data
                    .push(value.strip_prefix(' ').unwrap_or(value).to_string());
            }
            if self.data.iter().map(String::len).sum::<usize>() > 2 * 1024 * 1024 {
                return Err("provider SSE event exceeded limit".into());
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
