use super::*;

fn checked(raw: &[u8], expected_nodes: usize) -> OriginalJson {
    let value = OriginalJson::parse(raw).unwrap();
    assert_eq!(value.as_bytes(), raw, "original bytes must be preserved");
    assert_eq!(value.node_count(), expected_nodes, "independent node count");
    value
}

fn refused(raw: &[u8], reason: &str) {
    let result = OriginalJson::parse(raw);
    assert!(result.is_err(), "expected source JSON refusal: {reason}");
    match result {
        Err(AppError::Other(detail)) => assert_eq!(detail, "source_json_invalid"),
        Err(other) => panic!("expected static invalid refusal, got {other:?}"),
        Ok(_) => unreachable!(),
    }
}

#[test]
fn original_bytes_and_opaque_numeric_tokens_are_preserved() {
    for number in [
        "0",
        "-0",
        "1.00",
        "0.0036944444444444438",
        "0.3333333333333333",
        "1e999",
        "-1E-9999",
        "184467440737095516160000000000000000000000000001",
        "2.2250738585072014e-308",
        "5e-324",
    ] {
        let raw = format!(" \r\n\t{number} \n");
        checked(raw.as_bytes(), 1);
        let raw = format!(" \n{{\"n\":{number},\"token\":\"{number}\"}}\t\r ");
        checked(raw.as_bytes(), 3);
    }
}

#[test]
fn all_six_existing_numeric_audit_definitions_keep_exact_original_text() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/research/numeric-json-audit-v1.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for case in cases {
        // The fixture carries the definition as a JSON string, not parsed floats.
        checked(case["definitionJson"].as_str().unwrap().as_bytes(), 6);
    }
}

#[test]
fn scalar_roots_and_independent_mixed_tree_counts() {
    for raw in ["null", "true", "false", "42", "\"text\"", "[]", "{}"] {
        checked(raw.as_bytes(), 1);
    }
    checked(
        br#"{"a":[null,true,false,{"b":"x"}],"c":{},"d":[[],1,2.0,-0]}"#,
        13,
    );
    checked(br#"[{},[],[null],{"a":[],"b":{}}]"#, 8);
}

#[test]
fn decoded_duplicate_keys_refuse_at_every_container_location() {
    for raw in [
        r#"{"a":1,"a":2}"#,
        r#"{"a":1,"\u0061":2}"#,
        r#"{"\u0061":1,"a":2}"#,
        r#"{"":null,"":false}"#,
        r#"{"outer":{"key":1,"k\u0065y":2}}"#,
        r#"[0,{"a":[{"q":null,"\u0071":true}]}]"#,
        r#"{"\ud83d\ude00":1,"😀":2}"#,
        r#"{"a":null,"a":{"deep":[1,2]}}"#,
    ] {
        refused(raw.as_bytes(), "decoded duplicate must refuse");
    }
}

#[test]
fn key_scope_and_unicode_normalization_are_not_conflated() {
    checked(br#"{"a":1,"child":{"a":2},"rows":[{"a":3},{"a":4}]}"#, 9);
    // Precomposed and decomposed keys are distinct scalar sequences.
    checked("{\"é\":1,\"e\\u0301\":2}".as_bytes(), 3);
    checked(br#"{"A":1,"a":2,"a ":3,"":4}"#, 5);
}

#[test]
fn unicode_string_scalars_and_escapes_are_validated_without_rewriting() {
    for raw in [
        r#""\ud83d\ude00""#,
        "\"中文😀\"",
        r#""\u0000\n\t\r\b\f\\\/\"""#,
        r#"{"\u0061":"\u0062","😀":["é","e\u0301"]}"#,
    ] {
        let expected = if raw.starts_with('{') { 5 } else { 1 };
        checked(raw.as_bytes(), expected);
    }
    for raw in [
        r#""\ud800""#,
        r#""\udc00""#,
        r#""\ud800x""#,
        r#"{"\ud800":0}"#,
        r#"{"a":["\udfff"]}"#,
        r#""\ud800\u0061""#,
    ] {
        refused(raw.as_bytes(), "invalid Unicode scalar must refuse");
    }
}

#[test]
fn malformed_syntax_and_trailing_values_use_static_errors() {
    for raw in [
        "",
        " ",
        "null null",
        "{}[]",
        "true!",
        "[1,]",
        "{\"a\":1,}",
        "[",
        "{",
        "[}",
        "{]",
        "}",
        "]",
        "{a:1}",
        "{\"a\" 1}",
        "[1 2]",
        "//comment\nnull",
        "/*comment*/null",
        "NaN",
        "Infinity",
        "-Infinity",
        "+1",
        "01",
        "-01",
        "1.",
        ".1",
        "1e",
        "1e+",
        "--1",
        "\"unterminated",
        "\"\\x00\"",
        "\"\\u12\"",
        "\"line\nfeed\"",
        "\u{feff}null",
        "null\u{a0}",
        "null\0",
    ] {
        refused(raw.as_bytes(), "malformed/trailing syntax");
    }
    // Diagnostics cannot echo the original sensitive content.
    refused(br#"{"private-source":"do-not-echo",}"#, "static diagnostic");
}

#[test]
fn invalid_utf8_and_sql_like_binary_payloads_refuse() {
    for raw in [
        &[0xff][..],
        b"\"\xc0\xaf\"",
        b"\"\xed\xa0\x80\"",
        b"{\"\xff\":1}",
        b"[\"\xf0\x28\x8c\xbc\"]",
        b"\x00null",
    ] {
        refused(raw, "invalid UTF-8/binary");
    }
}

#[test]
fn exact_four_mib_original_byte_cap_counts_whitespace_and_utf8() {
    let mut padded = vec![b' '; 4_194_304];
    padded[..4].copy_from_slice(b"null");
    checked(&padded, 1);
    padded.push(b' ');
    refused(&padded, "byte cap plus one must refuse");

    let text = format!("\"{}aa\"", "界".repeat(1_398_100));
    assert_eq!(text.len(), 4_194_304);
    checked(text.as_bytes(), 1);
    let mut oversized = text.into_bytes();
    oversized.push(b'\n');
    refused(&oversized, "multibyte original byte cap plus one");
}

#[test]
fn exact_flat_node_cap_and_one_over_are_independent_of_byte_size() {
    let mut flat = format!("[{}]", vec!["null"; 65_535].join(","));
    checked(flat.as_bytes(), 65_536); // root + 65,535 scalar leaves.
    flat.pop();
    flat.push_str(",null]");
    refused(flat.as_bytes(), "node cap plus one must refuse");
}

#[test]
fn exact_branching_node_cap_counts_containers_and_null_values() {
    let mut branches = format!("[{}]", vec!["[null,true]"; 21_845].join(","));
    checked(branches.as_bytes(), 65_536); // root + 21,845 * (array + 2).
    branches.pop();
    branches.push_str(",false]");
    refused(
        branches.as_bytes(),
        "branching node cap plus one must refuse",
    );

    let objects = format!("[{},null]", vec![r#"{"k":null}"#; 32_767].join(","));
    checked(objects.as_bytes(), 65_536); // root + 32,767 * 2 + final null.
}

#[test]
fn exact_depth_counts_root_scalars_and_empty_containers() {
    let scalar_at_64 = format!("{}null{}", "[".repeat(63), "]".repeat(63));
    checked(scalar_at_64.as_bytes(), 64);
    let scalar_at_65 = format!("[{scalar_at_64}]");
    refused(scalar_at_65.as_bytes(), "depth cap plus one must refuse");

    let empty_at_64 = format!("{}{}", "[".repeat(64), "]".repeat(64));
    checked(empty_at_64.as_bytes(), 64);
    refused(
        format!("[{empty_at_64}]").as_bytes(),
        "empty depth plus one",
    );

    let object_at_64 = format!("{}null{}", r#"{"k":"#.repeat(63), "}".repeat(63));
    checked(object_at_64.as_bytes(), 64);
    refused(
        format!("[{object_at_64}]").as_bytes(),
        "mixed depth plus one",
    );
}

#[test]
fn decoded_key_byte_cap_counts_escapes_and_multibyte_scalars() {
    for key in [
        "a".repeat(1024),
        r"\u0061".repeat(1024),
        format!("{}a", "界".repeat(341)),
    ] {
        let raw = format!("{{\"{key}\":null}}");
        checked(raw.as_bytes(), 2);
        refused(
            format!("{{\"{key}a\":null}}").as_bytes(),
            "key cap plus one",
        );
    }
    let emoji = "😀".repeat(256);
    checked(format!("{{\"{emoji}\":[]}}").as_bytes(), 2);
    refused(
        format!("{{\"{emoji}a\":[]}}").as_bytes(),
        "emoji byte cap plus one",
    );
}

#[test]
fn quote_aware_depth_scan_ignores_string_brackets_and_bounds_deep_input() {
    let string = format!("\"{}\"", r#"[{{\"\\\"]}}"#.repeat(2000));
    checked(string.as_bytes(), 1);
    let nested = format!("[{{\"key\":{string}}}]");
    checked(nested.as_bytes(), 3);
    let deep = format!("{}0{}", "[".repeat(10_000), "]".repeat(10_000));
    refused(deep.as_bytes(), "deep input before parser scratch growth");
    let malformed_deep = "{".repeat(10_000);
    refused(malformed_deep.as_bytes(), "deep malformed input");
}

#[test]
fn observations_own_original_bytes_and_clones_after_input_mutation() {
    let mut input = b" \n{\"x\":-0}\r ".to_vec();
    let value = checked(&input, 2);
    let clone = value.clone();
    input.fill(b'x');
    assert_eq!(value.as_bytes(), b" \n{\"x\":-0}\r ");
    assert!(value == clone);
    assert_eq!(clone.node_count(), 2);
    assert!(value != checked(br#"{"x":-0}"#, 2));
}
