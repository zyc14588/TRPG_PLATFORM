api_call() {
  local method="$1" path="$2" headers="$3" body="$4" output="$5"
  local -a arguments=(--silent --show-error --cacert "$runtime/ca.crt" --request "$method" --header "@$headers" --output "$output" --write-out '%{http_code}')
  [[ -z "$body" ]] || arguments+=(--header 'Content-Type: application/json' --data-binary "@$body")
  curl "${arguments[@]}" "$api_base/$path"
}
product_api_call() {
  local method="$1" path="$2" headers="$3" body="$4" output="$5"
  local -a arguments=(--silent --show-error --cacert "$runtime/ca.crt" --request "$method" --header "@$headers" --output "$output" --write-out '%{http_code}')
  [[ -z "$body" ]] || arguments+=(--header 'Content-Type: application/json' --data-binary "@$body")
  curl "${arguments[@]}" "$product_api_base/$path"
}
json_value() { python3 - "$1" "$2" <<'PY'
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
for part in sys.argv[2].split("."):
    value = value[part]
print(str(value).lower() if isinstance(value, bool) else value)
PY
}
response_error() { python3 - "$1" <<'PY'
import json, sys
try:
    value = json.load(open(sys.argv[1], encoding="utf-8"))
    print(value.get("error", value.get("code", "UNKNOWN_RESPONSE")))
except (OSError, ValueError, AttributeError):
    print("INVALID_RESPONSE")
PY
}
