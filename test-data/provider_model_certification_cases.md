# Test Data — Provider / Model Certification Cases

```json
{
  "providers": [
    {"provider_type":"ollama","base_url":"http://localhost:11434/v1","api_key":"ollama","env":"dev","expected_health":"ok"},
    {"provider_type":"llama_cpp","base_url":"http://localhost:8080/v1","api_key":"sk-no-key-required","env":"dev","expected_health":"ok"},
    {"provider_type":"local_openai_compatible","base_url":"http://0.0.0.0:11434/v1","api_key":"ollama","env":"prod","expected_health":"deny","reason":"unauthenticated_local_provider_exposed"}
  ],
  "certification_requests": [
    {
      "request":{"request_id":"unstable-run","model_id":"unstable-chat","model_artifact_sha256":"sha256:1111111111111111111111111111111111111111111111111111111111111111","provider_id":"ollama","provider_type":"ollama","provider_runtime_sha256":"sha256:2222222222222222222222222222222222222222222222222222222222222222","suite_id":"local-ai-keeper-certification","suite_version":"1.0.0"},
      "fake_provider_transcript":"tool_instability",
      "expected_run_status":"failed",
      "expected_failed_case":"tool_use_stability",
      "expected_level":"LOCAL_MODEL_LEVEL_3"
    },
    {
      "request":{"request_id":"stable-run","model_id":"json-tool-stable","model_artifact_sha256":"sha256:3333333333333333333333333333333333333333333333333333333333333333","provider_id":"ollama","provider_type":"ollama","provider_runtime_sha256":"sha256:4444444444444444444444444444444444444444444444444444444444444444","suite_id":"local-ai-keeper-certification","suite_version":"1.0.0"},
      "fake_provider_transcript":"eight_case_pass_v1",
      "expected_cases":["capability_probe","golden","tool_use_stability","visibility_leakage","prompt_injection","coc_rules_mini_eval","latency","context_stress"],
      "expected_run_status":"passed",
      "expected_level":"LOCAL_MODEL_LEVEL_4"
    }
  ],
  "certificate_signature_bindings": [
    "model_artifact_sha256","provider_id","provider_type","provider_runtime_sha256","suite_id","suite_version","suite_sha256","prompt_set_sha256","tool_schema_sha256","ruleset_sha256","policy_sha256","evidence_sha256"
  ],
  "fallback": [
    {"local_model":"json-tool-stable","cloud_fallback_enabled":false,"cloud_call_attempted":true,"expected":"DENY_AND_AUDIT"},
    {"local_model":"json-tool-stable","cloud_fallback_enabled":true,"user_notice":true,"snapshot_recorded":true,"expected":"ALLOW"}
  ]
}
```
