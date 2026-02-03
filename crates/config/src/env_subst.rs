/// Replace `${ENV_VAR}` placeholders in config string values.
///
/// Unresolvable variables are left as-is.
pub fn substitute_env(input: &str) -> String {
    substitute_env_with(input, |name| std::env::var(name).ok())
}

/// Replace `${ENV_VAR}` placeholders using a custom lookup function.
///
/// This is the implementation used by [`substitute_env`]; the separate
/// signature makes it testable without mutating the process environment.
fn substitute_env_with(input: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut var_name = String::new();
            let mut closed = false;
            for c in chars.by_ref() {
                if c == '}' {
                    closed = true;
                    break;
                }
                var_name.push(c);
            }
            if closed && !var_name.is_empty() {
                match lookup(&var_name) {
                    Some(val) => result.push_str(&val),
                    None => {
                        // Leave unresolved placeholder as-is.
                        result.push_str("${");
                        result.push_str(&var_name);
                        result.push('}');
                    },
                }
            } else {
                // Malformed — emit literal.
                result.push_str("${");
                result.push_str(&var_name);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_known_var() {
        let lookup = |name: &str| match name {
            "MOLTIS_TEST_VAR" => Some("hello".to_string()),
            _ => None,
        };
        assert_eq!(
            substitute_env_with("key=${MOLTIS_TEST_VAR}", lookup),
            "key=hello"
        );
    }

    #[test]
    fn leaves_unknown_var() {
        let lookup = |_: &str| None;
        assert_eq!(
            substitute_env_with("${MOLTIS_NONEXISTENT_XYZ}", lookup),
            "${MOLTIS_NONEXISTENT_XYZ}"
        );
    }

    #[test]
    fn no_placeholders() {
        assert_eq!(substitute_env("plain text"), "plain text");
    }
}
