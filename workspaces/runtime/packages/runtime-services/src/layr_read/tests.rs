use super::*;
use studio_json::json;

/// The exact allowlist and argument rules of Python `run_layr_read`.
#[test]
fn layr_read_keeps_the_python_argument_rules() {
    let accepted = |args: Value| command(&json!({"args": args}));
    for args in [
        json!(["status"]),
        json!(["show", "s1:src/main.rs"]),
        json!(["--line", "main", "log"]),
        json!(["--line", "a", "--line", "b", "ls-files"]),
        json!(["blame", "--", "x"]),
        json!(["grep", "-n", "x"]),
        json!(["merge-base", "a", "b"]),
        json!(["rev-parse", "head"]),
        json!(["diff", "x".repeat(4096)]),
        Value::Array(std::iter::repeat_n(json!("status"), 64).collect()),
    ] {
        assert_eq!(
            accepted(args.clone()),
            Ok(studio_json::from_value(args).unwrap())
        );
    }
    let shape = "Supply args as 1 to 64 strings";
    let allowlist = "layr_read runs only blame, diff, grep, log, ls-files, merge-base, \
                     rev-parse, show, status, after optional --line <line>";
    for (args, refusal) in [
        (json!([]), shape),
        (json!("status"), shape),
        (json!(["status", 1]), shape),
        (json!(["show", "a\u{0}b"]), shape),
        (json!(["diff", "x".repeat(4097)]), shape),
        (
            Value::Array(std::iter::repeat_n(json!("status"), 65).collect()),
            shape,
        ),
        (json!(["try", "sh"]), allowlist),
        (json!(["--project", "p", "status"]), allowlist),
        (json!(["-C", "/", "status"]), allowlist),
        (json!(["--line"]), allowlist),
        (json!(["--line", "main"]), allowlist),
        (json!(["commit"]), allowlist),
    ] {
        assert_eq!(accepted(args.clone()), Err(refusal.to_owned()), "{args}");
    }
    assert_eq!(command(&json!({})), Err(shape.to_owned()));
}
