use super::*;

fn options(kind: Kind) -> RemoteStateOptions {
    RemoteStateOptions {
        kind,
        source: "a".repeat(40),
        tag: "v0.1.1".into(),
        input: "-".into(),
        query_exit: 0,
        allow_missing: false,
    }
}

fn response(body: &str) -> String {
    format!("HTTP/2.0 200 OK\r\nContent-Type: application/json\r\n\r\n{body}")
}

#[test]
fn tag_requires_exact_full_source_and_requested_ref() {
    let mut options = options(Kind::Tag);
    let valid = response(&format!(
        r#"{{"ref":"refs/tags/v0.1.1","object":{{"sha":"{}"}}}}"#,
        options.source
    ));
    assert!(verify(&options, &valid).unwrap());
    for invalid in [
        valid.replace(&options.source, "abc"),
        valid.replace(&options.source, &"b".repeat(40)),
        valid.replace("refs/tags/v0.1.1", "refs/heads/v0.1.1"),
        valid.replace("v0.1.1", "v0.2.0"),
        response("{}"),
        valid.replace("\"ref\":", "\"ref\":null,\"ref\":"),
    ] {
        assert!(verify(&options, &invalid).is_err(), "{invalid}");
    }
    for source in ["", "abc", &"g".repeat(40)] {
        options.source = source.into();
        assert!(verify(&options, &valid).is_err());
    }
}

#[test]
fn draft_requires_a_present_matching_unpublished_release_and_no_graphql_errors() {
    let options = options(Kind::Draft);
    let valid =
        response(r#"{"data":{"repository":{"release":{"tagName":"v0.1.1","isDraft":true}}}}"#);
    assert!(verify(&options, &valid).unwrap());
    for invalid in [
        valid.replace("true", "false"),
        valid.replace("true", "\"true\""),
        valid.replace("true", "null"),
        valid.replace("v0.1.1", "v0.2.0"),
        valid.replace("\"isDraft\":true", "\"isDraft\":true,\"isDraft\":false"),
        valid.replace(
            "\"data\":",
            "\"errors\":[{\"message\":\"denied\"}],\"data\":",
        ),
        response(r#"{"data":{"repository":null}}"#),
        response(r#"{"data":null}"#),
        response(r#"{"data":{"repository":{"release":null}}}"#),
    ] {
        assert!(verify(&options, &invalid).is_err(), "{invalid}");
    }
}

#[test]
fn only_explicit_absence_is_allowed_and_transport_failures_cannot_become_creation() {
    for kind in [Kind::Tag, Kind::Draft] {
        let mut options = options(kind);
        options.allow_missing = true;
        let missing = if kind == Kind::Tag {
            options.query_exit = 1;
            "HTTP/2.0 404 Not Found\n\n{\"message\":\"Not Found\"}".to_owned()
        } else {
            response(r#"{"data":{"repository":{"release":null}}}"#)
        };
        assert!(!verify(&options, &missing).unwrap());
        options.allow_missing = false;
        assert!(verify(&options, &missing).is_err());
        options.allow_missing = true;
        for code in ["401", "403", "429", "500", "502"] {
            assert!(verify(&options, &format!("HTTP/1.1 {code}\n\n{{}}")).is_err());
        }
        for malformed in ["", "network unavailable", "HTTP/2.0 200 OK", "200\n\n{}"] {
            assert!(verify(&options, malformed).is_err());
        }
        options.query_exit = 23;
        assert!(verify(&options, "HTTP/2.0 404 Not Found\n\n{}").is_err());
        assert!(
            verify(
                &options,
                &response(r#"{"data":{"repository":{"release":null}}}"#)
            )
            .is_err()
        );
    }
}
