use super::*;

fn chunk(delta: Value, finish: Value) -> Value {
    json!({"choices":[{"index":0,"delta":delta,"finish_reason":finish}]})
}
fn stream() -> ChatStream {
    ChatStream::new("fixture-model".into(), HashSet::new())
}

#[test]
fn converts_system_user_assistant_and_reasoning_effort_without_mapping_it() {
    let converted=convert_request(&json!({"model":"MiniMax-M3","instructions":"system instruction","input":[{"role":"developer","content":[{"type":"input_text","text":"developer instruction"}]},{"role":"user","content":[{"type":"input_text","text":"hello"}]}],"reasoning":{"effort":"ultra"},"max_output_tokens":128})).unwrap();
    assert_eq!(converted.body["model"], "MiniMax-M3");
    assert_eq!(
        converted.body["messages"],
        json!([{"role":"system","content":"system instruction"},{"role":"system","content":"developer instruction"},{"role":"user","content":"hello"}])
    );
    assert_eq!(converted.body["reasoning_effort"], "ultra");
    assert_eq!(converted.body["max_tokens"], 128);
}

#[test]
fn custom_and_function_tools_preserve_history_and_tool_call_ids() {
    let request = json!({"model":"fixture","tools":[{"type":"custom","name":"apply_patch","description":"patch exactly"},{"type":"function","name":"exec_command","parameters":{"type":"object","properties":{"cmd":{"type":"string"}}}}],"input":[{"type":"custom_tool_call","call_id":"call-1","name":"apply_patch","input":"*** Begin Patch\n*** End Patch"},{"type":"custom_tool_call_output","call_id":"call-1","output":"ok"}]});
    let converted = convert_request(&request).unwrap();
    assert!(converted.custom_tools.contains("apply_patch"));
    assert_eq!(
        converted.body["tools"][1]["function"]["name"],
        "exec_command"
    );
    assert_eq!(
        converted.body["messages"][0]["tool_calls"][0]["id"],
        "call-1"
    );
    assert_eq!(
        converted.body["messages"][1],
        json!({"role":"tool","tool_call_id":"call-1","content":"ok"})
    );
}

#[test]
fn images_keep_their_url_and_detail() {
    let body=convert_request(&json!({"model":"fixture","input":[{"role":"user","content":[{"type":"input_text","text":"look"},{"type":"input_image","image_url":"data:image/png;base64,fixture","detail":"high"}]}]})).unwrap().body;
    assert_eq!(
        body["messages"][0]["content"][1],
        json!({"type":"image_url","image_url":{"url":"data:image/png;base64,fixture","detail":"high"}})
    );
}

#[test]
fn unsupported_content_and_hosted_tools_fail_explicitly() {
    assert!(
        convert_request(&json!({"model":"fixture","input":[{"type":"computer_call"}]})).is_err()
    );
    assert!(convert_request(
        &json!({"model":"fixture","input":"hello","tools":[{"type":"web_search"}]})
    )
    .is_err());
    assert!(convert_request(&json!({"input":"hello"})).is_err());
}

#[test]
fn sse_decoder_handles_every_byte_boundary_and_crlf() {
    let payload = "event: chat\r\ndata: {\"text\":\"中文🙂\"}\r\n\r\ndata: [DONE]\n\n";
    let mut decoder = SseDecoder::default();
    let mut output = Vec::new();
    for byte in payload.as_bytes() {
        output.extend(decoder.push(std::slice::from_ref(byte)).unwrap());
    }
    assert_eq!(output, vec!["{\"text\":\"中文🙂\"}", "[DONE]"]);
}

#[test]
fn sse_decoder_ignores_comments_and_joins_multiline_events() {
    let mut decoder = SseDecoder::default();
    assert_eq!(
        decoder
            .push(b": keepalive\ndata: {\ndata: \"choices\": []\ndata: }\n\n")
            .unwrap(),
        vec!["{\n\"choices\": []\n}"]
    );
}

#[test]
fn sse_invalid_utf8_and_oversized_frames_are_rejected() {
    assert!(SseDecoder::default().push(b"data: \xff\n\n").is_err());
    assert!(SseDecoder::default()
        .push(&vec![b'x'; 2 * 1024 * 1024 + 1])
        .is_err());
}

#[test]
fn genuine_text_deltas_then_done_become_a_completed_response() {
    let mut adapter = stream();
    let initial = adapter.begin();
    assert_eq!(initial[0]["type"], "response.created");
    let first = adapter
        .push(&chunk(
            json!({"role":"assistant","content":"hello "}),
            Value::Null,
        ))
        .unwrap();
    assert!(first
        .iter()
        .any(|v| v["type"] == "response.output_text.delta" && v["delta"] == "hello "));
    adapter
        .push(&chunk(json!({"content":"world"}), json!("stop")))
        .unwrap();
    adapter.push(&json!({"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7}})).unwrap();
    let final_events = adapter.finish().unwrap();
    let end = final_events.last().unwrap();
    assert_eq!(end["type"], "response.completed");
    assert_eq!(
        end["response"]["output"][0]["content"][0]["text"],
        "hello world"
    );
    assert_eq!(
        end["response"]["usage"],
        json!({"input_tokens":3,"output_tokens":4,"total_tokens":7})
    );
    assert!(
        initial[0]["sequence_number"].as_u64().unwrap() < end["sequence_number"].as_u64().unwrap()
    );
}

#[test]
fn absence_of_finish_reason_or_upstream_error_never_claims_completion() {
    let mut adapter = stream();
    adapter.begin();
    adapter
        .push(&chunk(json!({"content":"partial"}), Value::Null))
        .unwrap();
    assert!(adapter.finish().is_err());
    assert!(adapter
        .push(&json!({"error":{"message":"provider error"}}))
        .is_err());
    assert_eq!(
        adapter.fail("interrupted upstream")["type"],
        "response.failed"
    );
}

#[test]
fn token_truncation_is_incomplete_and_cannot_release_tool_calls() {
    let mut adapter = stream();
    adapter.begin();
    adapter.push(&chunk(json!({"tool_calls":[{"index":0,"id":"call1","function":{"name":"exec_command","arguments":"{}"}}]}),json!("length"))).unwrap();
    let final_events = adapter.finish().unwrap();
    assert!(!final_events
        .iter()
        .any(|v| v["type"] == "response.output_item.done" && v["item"]["type"] == "function_call"));
    assert_eq!(final_events.last().unwrap()["type"], "response.incomplete");
}

#[test]
fn fragmented_tools_are_emitted_only_after_terminal_confirmation() {
    let mut adapter = stream();
    adapter.begin();
    let first=adapter.push(&chunk(json!({"tool_calls":[{"index":0,"id":"call1","function":{"name":"exec_command","arguments":"{\"cmd\":"}}]}),Value::Null)).unwrap();
    assert!(first.is_empty());
    adapter
        .push(&chunk(
            json!({"tool_calls":[{"index":0,"function":{"arguments":"\"echo ok\"}"}}]}),
            json!("tool_calls"),
        ))
        .unwrap();
    let final_events = adapter.finish().unwrap();
    let end = final_events.last().unwrap();
    assert_eq!(end["response"]["output"][0]["name"], "exec_command");
    assert_eq!(end["response"]["output"][0]["call_id"], "call1");
    assert_eq!(
        end["response"]["output"][0]["arguments"],
        "{\"cmd\":\"echo ok\"}"
    );
}

#[test]
fn custom_patch_input_is_unwrapped_not_reinvented() {
    let patch = "*** Begin Patch\n*** Add File: hello.txt\n+hello\n*** End Patch";
    let mut adapter = ChatStream::new("fixture".into(), HashSet::from(["apply_patch".into()]));
    adapter.begin();
    adapter.push(&chunk(json!({"tool_calls":[{"index":0,"id":"patch1","function":{"name":"apply_patch","arguments":json!({"input":patch}).to_string()}}]}),json!("tool_calls"))).unwrap();
    let final_events = adapter.finish().unwrap();
    let item = &final_events.last().unwrap()["response"]["output"][0];
    assert_eq!(item["type"], "custom_tool_call");
    assert_eq!(item["input"], patch);
}

#[test]
fn multiple_tools_keep_distinct_ids_and_order() {
    let mut adapter = stream();
    adapter.begin();
    adapter.push(&chunk(json!({"tool_calls":[{"index":1,"id":"second","function":{"name":"two","arguments":"{}"}},{"index":0,"id":"first","function":{"name":"one","arguments":"{}"}}]}),json!("tool_calls"))).unwrap();
    let final_events = adapter.finish().unwrap();
    let output = final_events.last().unwrap()["response"]["output"]
        .as_array()
        .unwrap();
    assert_eq!(output[0]["call_id"], "first");
    assert_eq!(output[1]["call_id"], "second");
}

#[test]
fn namespaced_tools_round_trip_without_losing_dispatch_identity() {
    let converted=convert_request(&json!({"model":"fixture","tools":[{"type":"namespace","name":"functions","description":"local tools","tools":[{"type":"function","name":"exec_command","parameters":{"type":"object"}}]}],"input":[{"type":"function_call","namespace":"functions","name":"exec_command","call_id":"earlier","arguments":"{}"},{"type":"function_call_output","call_id":"earlier","output":"done"}]})).unwrap();
    assert_eq!(
        converted.body["tools"][0]["function"]["name"],
        "functions__exec_command"
    );
    assert_eq!(
        converted.body["messages"][0]["tool_calls"][0]["function"]["name"],
        "functions__exec_command"
    );
    let mut adapter = ChatStream::new("fixture".into(), converted.custom_tools)
        .with_tool_names(converted.tool_names);
    adapter.begin();
    adapter.push(&chunk(json!({"tool_calls":[{"index":0,"id":"call1","function":{"name":"functions__exec_command","arguments":"{}"}}]}),json!("tool_calls"))).unwrap();
    let events = adapter.finish().unwrap();
    let item = &events.last().unwrap()["response"]["output"][0];
    assert_eq!(item["namespace"], "functions");
    assert_eq!(item["name"], "exec_command");
}
