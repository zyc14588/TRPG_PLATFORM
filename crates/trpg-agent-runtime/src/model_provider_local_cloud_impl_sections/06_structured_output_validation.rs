const MAX_STRUCTURED_SCHEMA_DEPTH: usize = 64;

fn validate_structured_output(schema: &Value, output: &Value) -> bool {
    validate_structured_value(schema, output, 0)
}

fn validate_structured_value(schema: &Value, output: &Value, depth: usize) -> bool {
    if depth > MAX_STRUCTURED_SCHEMA_DEPTH {
        return false;
    }
    if let Some(allowed) = schema.as_bool() {
        return allowed;
    }
    let Some(schema) = schema.as_object() else {
        return false;
    };
    const SUPPORTED: &[&str] = &[
        "type",
        "properties",
        "required",
        "additionalProperties",
        "const",
        "enum",
        "anyOf",
        "allOf",
        "items",
        "minItems",
        "maxItems",
        "minLength",
        "maxLength",
        "minimum",
        "maximum",
        "title",
        "description",
    ];
    if schema.keys().any(|key| !SUPPORTED.contains(&key.as_str())) {
        return false;
    }
    if schema
        .get("const")
        .is_some_and(|constant| constant != output)
        || schema.get("enum").is_some_and(|values| {
            !values
                .as_array()
                .is_some_and(|values| !values.is_empty() && values.contains(output))
        })
    {
        return false;
    }
    if let Some(any_of) = schema.get("anyOf") {
        let Some(any_of) = any_of.as_array() else {
            return false;
        };
        if any_of.is_empty()
            || !any_of
                .iter()
                .any(|candidate| validate_structured_value(candidate, output, depth + 1))
        {
            return false;
        }
    }
    if let Some(all_of) = schema.get("allOf") {
        let Some(all_of) = all_of.as_array() else {
            return false;
        };
        if all_of.is_empty()
            || !all_of
                .iter()
                .all(|candidate| validate_structured_value(candidate, output, depth + 1))
        {
            return false;
        }
    }
    if !schema
        .get("type")
        .is_none_or(|kind| structured_type_matches(kind, output))
    {
        return false;
    }
    validate_structured_object(schema, output, depth)
        && validate_structured_array(schema, output, depth)
        && validate_structured_scalar(schema, output)
}

fn structured_type_matches(kind: &Value, output: &Value) -> bool {
    let matches = |kind: &str| match kind {
        "null" => output.is_null(),
        "boolean" => output.is_boolean(),
        "object" => output.is_object(),
        "array" => output.is_array(),
        "number" => output.is_number(),
        "integer" => output.as_i64().is_some() || output.as_u64().is_some(),
        "string" => output.is_string(),
        _ => false,
    };
    kind.as_str().is_some_and(matches)
        || kind.as_array().is_some_and(|kinds| {
            !kinds.is_empty()
                && kinds
                    .iter()
                    .all(Value::is_string)
                && kinds.iter().filter_map(Value::as_str).any(matches)
        })
}

fn validate_structured_object(schema: &Map<String, Value>, output: &Value, depth: usize) -> bool {
    let has_object_rules = schema.contains_key("properties")
        || schema.contains_key("required")
        || schema.contains_key("additionalProperties");
    if !has_object_rules {
        return true;
    }
    let Some(output) = output.as_object() else {
        return false;
    };
    let empty_properties = Map::new();
    let properties = match schema.get("properties") {
        Some(properties) => {
            let Some(properties) = properties.as_object() else {
                return false;
            };
            properties
        }
        None => &empty_properties,
    };
    let required_valid = schema.get("required").is_none_or(|required| {
        required.as_array().is_some_and(|required| {
            required.iter().all(|name| {
                name.as_str()
                    .is_some_and(|name| output.contains_key(name))
            })
        })
    });
    if !required_valid
        || !properties.iter().all(|(name, child_schema)| {
            output.get(name).is_none_or(|value| {
                validate_structured_value(child_schema, value, depth + 1)
            })
        })
    {
        return false;
    }
    match schema.get("additionalProperties") {
        Some(Value::Bool(false)) => output.keys().all(|name| properties.contains_key(name)),
        Some(Value::Bool(true)) | None => true,
        Some(additional_schema) if additional_schema.is_object() => {
            output.iter().all(|(name, value)| {
                properties.contains_key(name)
                    || validate_structured_value(additional_schema, value, depth + 1)
            })
        }
        Some(_) => false,
    }
}

fn validate_structured_array(schema: &Map<String, Value>, output: &Value, depth: usize) -> bool {
    let has_array_rules = schema.contains_key("items")
        || schema.contains_key("minItems")
        || schema.contains_key("maxItems");
    if !has_array_rules {
        return true;
    }
    let Some(output) = output.as_array() else {
        return false;
    };
    validate_usize_bound(schema, "minItems", |minimum| output.len() >= minimum)
        && validate_usize_bound(schema, "maxItems", |maximum| output.len() <= maximum)
        && schema.get("items").is_none_or(|items| {
            (items.is_object() || items.is_boolean())
                && output
                    .iter()
                    .all(|value| validate_structured_value(items, value, depth + 1))
        })
}

fn validate_structured_scalar(schema: &Map<String, Value>, output: &Value) -> bool {
    if let Some(output) = output.as_str() {
        let length = output.chars().count();
        if !validate_usize_bound(schema, "minLength", |minimum| length >= minimum)
            || !validate_usize_bound(schema, "maxLength", |maximum| length <= maximum)
        {
            return false;
        }
    } else if schema.contains_key("minLength") || schema.contains_key("maxLength") {
        return false;
    }
    if schema.contains_key("minimum") || schema.contains_key("maximum") {
        let Some(output) = output.as_f64() else {
            return false;
        };
        return validate_f64_bound(schema, "minimum", |minimum| output >= minimum)
            && validate_f64_bound(schema, "maximum", |maximum| output <= maximum);
    }
    true
}

fn validate_usize_bound(
    schema: &Map<String, Value>,
    key: &str,
    predicate: impl FnOnce(usize) -> bool,
) -> bool {
    match schema.get(key) {
        None => true,
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .is_some_and(predicate),
    }
}

fn validate_f64_bound(
    schema: &Map<String, Value>,
    key: &str,
    predicate: impl FnOnce(f64) -> bool,
) -> bool {
    match schema.get(key) {
        None => true,
        Some(value) => value.as_f64().is_some_and(predicate),
    }
}
