# Test Data — Provider / Model Certification Cases

```json
{
  "providers": [
    {"provider_type":"ollama","base_url":"http://localhost:11434/v1","api_key":"ollama","env":"dev","expected_health":"ok"},
    {"provider_type":"llama_cpp","base_url":"http://localhost:8080/v1","api_key":"sk-no-key-required","env":"dev","expected_health":"ok"},
    {"provider_type":"local_openai_compatible","base_url":"http://0.0.0.0:11434/v1","api_key":"ollama","env":"prod","expected_health":"deny","reason":"unauthenticated_local_provider_exposed"}
  ],
  "certification": [
    {"model_id":"unstable-chat","json_schema_support":false,"tool_call_support":false,"visibility_tests":"fail","expected_level":"LOCAL_MODEL_LEVEL_1"},
    {"model_id":"json-tool-stable","json_schema_support":true,"tool_call_support":true,"visibility_tests":"pass","rules_eval":"pass","latency_ms":1800,"expected_level":"LOCAL_MODEL_LEVEL_4"}
  ],
  "fallback": [
    {"local_model":"json-tool-stable","cloud_fallback_enabled":false,"cloud_call_attempted":true,"expected":"DENY_AND_AUDIT"},
    {"local_model":"json-tool-stable","cloud_fallback_enabled":true,"user_notice":true,"snapshot_recorded":true,"expected":"ALLOW"}
  ]
}
```
