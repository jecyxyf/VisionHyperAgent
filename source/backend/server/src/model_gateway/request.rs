use super::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub struct ChatRequest {
    pub body: Value,
    pub custom_tools: HashSet<String>,
    pub tool_names: HashMap<String, (String, Option<String>)>,
}

pub fn convert_request(input: &Value) -> Result<ChatRequest> {
    let model = input
        .get("model")
        .and_then(Value::as_str)
        .ok_or("model is required")?;
    let mut messages = Vec::<Value>::new();
    if let Some(instructions) = input
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        messages.push(json!({"role":"system","content":instructions}));
    }
    let items = match input.get("input") {
        Some(Value::String(text)) => vec![json!({"role":"user","content":text})],
        Some(Value::Array(items)) => items.clone(),
        _ => return Err("Responses input must be text or an array".into()),
    };
    for item in items {
        match item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("message")
        {
            "message" => {
                let role = match item.get("role").and_then(Value::as_str).unwrap_or("user") {
                    "developer" | "system" => "system",
                    "user" => "user",
                    "assistant" => "assistant",
                    _ => return Err("unsupported message role".into()),
                };
                let content = convert_content(item.get("content").unwrap_or(&Value::Null))?;
                messages.push(json!({"role":role,"content":content}));
            }
            "function_call" | "custom_tool_call" => {
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .ok_or("tool call is missing its ID")?;
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("tool call is missing its name")?;
                let name = wire_name(item.get("namespace").and_then(Value::as_str), name);
                let arguments = if item["type"] == "custom_tool_call" {
                    json!({"input":item.get("input").and_then(Value::as_str).unwrap_or("")})
                        .to_string()
                } else {
                    item.get("arguments")
                        .and_then(Value::as_str)
                        .ok_or("tool arguments must be a string")?
                        .to_owned()
                };
                if !messages.last().is_some_and(|m| m["role"] == "assistant") {
                    messages.push(json!({"role":"assistant","content":null,"tool_calls":[]}));
                }
                let message = messages.last_mut().expect("assistant message exists");
                if !message["tool_calls"].is_array() {
                    message["tool_calls"] = json!([]);
                }
                message["tool_calls"].as_array_mut().expect("array initialized").push(json!({"id":call_id,"type":"function","function":{"name":name,"arguments":arguments}}));
            }
            "function_call_output" | "custom_tool_call_output" => {
                let id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .ok_or("tool output is missing call_id")?;
                let output = item.get("output").cloned().unwrap_or(Value::Null);
                let content = if output.is_string() {
                    output
                } else if output.is_array() {
                    convert_content(&output)?
                } else {
                    json!(output.to_string())
                };
                messages.push(json!({"role":"tool","tool_call_id":id,"content":content}));
            }
            "reasoning" => {
                let thought = item
                    .get("summary")
                    .and_then(Value::as_array)
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|p| p.get("text").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                if !thought.is_empty() {
                    if !messages.last().is_some_and(|m| m["role"] == "assistant") {
                        messages.push(json!({"role":"assistant","content":null}));
                    }
                    messages.last_mut().expect("assistant message")["reasoning_content"] =
                        json!(thought);
                }
            }
            _ => return Err("unsupported Responses input type in Chat compatibility mode".into()),
        }
    }
    let mut tools = Vec::new();
    let mut custom_tools = HashSet::new();
    let mut tool_names = HashMap::new();
    if let Some(definitions) = input.get("tools").and_then(Value::as_array) {
        flatten_tools(
            definitions,
            None,
            "",
            &mut tools,
            &mut custom_tools,
            &mut tool_names,
        )?;
    }
    let mut body = json!({"model":model,"messages":messages,"stream":true,"stream_options":{"include_usage":true},"n":1});
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }
    for field in ["temperature", "top_p", "parallel_tool_calls"] {
        if let Some(value) = input.get(field) {
            body[field] = value.clone();
        }
    }
    if let Some(limit) = input.get("max_output_tokens") {
        body["max_tokens"] = limit.clone();
    }
    if let Some(effort) = input.pointer("/reasoning/effort") {
        body["reasoning_effort"] = effort.clone();
    }
    if let Some(choice) = input.get("tool_choice") {
        body["tool_choice"] = if choice.is_string() {
            choice.clone()
        } else {
            let name = choice
                .get("name")
                .and_then(Value::as_str)
                .ok_or("unsupported tool_choice")?;
            let name = wire_name(choice.get("namespace").and_then(Value::as_str), name);
            json!({"type":"function","function":{"name":name}})
        };
    }
    if let Some(format) = input.pointer("/text/format") {
        if format["type"] == "json_schema" {
            let mut schema = format.clone();
            schema.as_object_mut().expect("object").remove("type");
            body["response_format"] = json!({"type":"json_schema","json_schema":schema});
        } else if format["type"] == "json_object" {
            body["response_format"] = format.clone();
        }
    }
    Ok(ChatRequest {
        body,
        custom_tools,
        tool_names,
    })
}

fn convert_content(content: &Value) -> Result<Value> {
    if content.is_string() || content.is_null() {
        return Ok(content.clone());
    }
    let parts = content.as_array().ok_or("invalid message content")?;
    let mut converted = Vec::new();
    for part in parts {
        match part.get("type").and_then(Value::as_str) {
            Some("input_text"|"output_text"|"text")=>converted.push(json!({"type":"text","text":part.get("text").and_then(Value::as_str).unwrap_or("")})),
            Some("input_image")=>{
                let url=part.get("image_url").and_then(Value::as_str).ok_or("image URL is required")?;
                let mut image=json!({"url":url});if let Some(detail)=part.get("detail"){image["detail"]=detail.clone();}
                converted.push(json!({"type":"image_url","image_url":image}));
            }
            _=>return Err("unsupported content part in Chat compatibility mode".into()),
        }
    }
    if converted.iter().all(|part| part["type"] == "text") {
        Ok(json!(converted
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")))
    } else {
        Ok(json!(converted))
    }
}

fn wire_name(namespace: Option<&str>, name: &str) -> String {
    let qualified = match namespace {
        Some(ns) => format!("{ns}__{name}"),
        None => name.to_owned(),
    };
    if qualified.len() <= 64
        && qualified
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
    {
        return qualified;
    }
    let prefix: String = qualified
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(44)
        .collect();
    let hash: String = Sha256::digest(qualified.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{prefix}_{hash}")
}

fn flatten_tools(
    definitions: &[Value],
    namespace: Option<&str>,
    namespace_description: &str,
    tools: &mut Vec<Value>,
    custom: &mut HashSet<String>,
    names: &mut HashMap<String, (String, Option<String>)>,
) -> Result<()> {
    for tool in definitions {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .ok_or("unsupported unnamed model tool")?;
        if tool["type"] == "namespace" {
            if namespace.is_some() {
                return Err("nested tool namespaces are not supported".into());
            }
            let inner = tool
                .get("tools")
                .and_then(Value::as_array)
                .ok_or("namespace tools must be an array")?;
            flatten_tools(
                inner,
                Some(name),
                tool.get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                tools,
                custom,
                names,
            )?;
            continue;
        }
        let alias = wire_name(namespace, name);
        if names
            .insert(
                alias.clone(),
                (name.to_owned(), namespace.map(str::to_owned)),
            )
            .is_some()
        {
            return Err("ambiguous tool aliases".into());
        }
        let description = format!(
            "{}\n{}",
            namespace_description,
            tool.get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
        );
        match tool.get("type").and_then(Value::as_str) {
            Some("function")=>tools.push(json!({"type":"function","function":{"name":alias,"description":description,"parameters":tool.get("parameters").cloned().unwrap_or(json!({"type":"object","properties":{}}))}})),
            Some("custom")=>{
                custom.insert(alias.clone());
                tools.push(json!({"type":"function","function":{"name":alias,"description":format!("{description}\nProvide the exact raw tool input in the input field."),"parameters":{"type":"object","properties":{"input":{"type":"string"}},"required":["input"],"additionalProperties":false}}}));
            }
            _=>return Err("provider-hosted tool type is not supported in Chat compatibility mode".into()),
        }
    }
    Ok(())
}
