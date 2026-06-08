use crate::{Point, Value};
use std::{borrow::Borrow, fmt::Write as _};

/// Resolve the points to line protocol format
pub(crate) fn line_serialization<'a>(
    points: impl IntoIterator<Item = impl Borrow<Point<'a>>>,
) -> String {
    let mut line = String::new();

    for point in points {
        let point: &Point = point.borrow();
        push_escaped_measurement(&mut line, &point.measurement);

        for (tag, value) in &point.tags {
            line.push(',');
            push_escaped_keys_and_tags(&mut line, tag);
            line.push('=');

            match value {
                Value::String(s) => push_escaped_keys_and_tags(&mut line, s),
                Value::Float(f) => push_display(&mut line, f),
                Value::Integer(i) => push_display(&mut line, i),
                Value::Boolean(b) => line.push_str(bool_str(*b)),
            }
        }

        let mut was_first = true;

        for (field, value) in &point.fields {
            line.push({
                if was_first {
                    was_first = false;
                    ' '
                } else {
                    ','
                }
            });
            push_escaped_keys_and_tags(&mut line, field);
            line.push('=');

            match value {
                Value::String(s) => push_escaped_string_field_value(&mut line, s),
                Value::Float(f) => push_display(&mut line, f),
                Value::Integer(i) => {
                    push_display(&mut line, i);
                    line.push('i')
                }
                Value::Boolean(b) => line.push_str(bool_str(*b)),
            }
        }

        if let Some(t) = point.timestamp {
            line.push(' ');
            push_display(&mut line, t);
        }

        line.push('\n')
    }

    line
}

#[inline]
pub(crate) fn quote_ident(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for ch in value.chars() {
        match ch {
            '\\' | '"' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            '\n' => quoted.push_str("\\n"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

#[inline]
pub(crate) fn quote_literal(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('\'');
    for ch in value.chars() {
        match ch {
            '\\' | '\'' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            _ => quoted.push(ch),
        }
    }
    quoted.push('\'');
    quoted
}

#[inline]
pub(crate) fn conversion(value: &str) -> String {
    let mut converted = String::with_capacity(value.len());
    for ch in value.chars() {
        if !matches!(ch, '\'' | '"' | '\\') {
            converted.push(ch);
        }
    }

    let trimmed = converted.trim();
    if trimmed.len() == converted.len() {
        converted
    } else {
        trimmed.to_string()
    }
}

#[inline]
fn bool_str(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

#[inline]
fn push_display(buffer: &mut String, value: impl std::fmt::Display) {
    write!(buffer, "{value}").expect("writing to a string cannot fail");
}

#[inline]
fn push_escaped_keys_and_tags(buffer: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            ',' | '=' | ' ' => {
                buffer.push('\\');
                buffer.push(ch);
            }
            _ => buffer.push(ch),
        }
    }
}

#[inline]
fn push_escaped_measurement(buffer: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            ',' | ' ' => {
                buffer.push('\\');
                buffer.push(ch);
            }
            _ => buffer.push(ch),
        }
    }
}

#[inline]
fn push_escaped_string_field_value(buffer: &mut String, value: &str) {
    buffer.push('"');
    for ch in value.chars() {
        match ch {
            '\\' | '"' => {
                buffer.push('\\');
                buffer.push(ch);
            }
            _ => buffer.push(ch),
        }
    }
    buffer.push('"');
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Point, Points};

    #[inline]
    fn escape_keys_and_tags(value: impl AsRef<str>) -> String {
        let value = value.as_ref();
        let mut escaped = String::with_capacity(value.len());
        push_escaped_keys_and_tags(&mut escaped, value);
        escaped
    }

    #[inline]
    fn escape_measurement(value: &str) -> String {
        let mut escaped = String::with_capacity(value.len());
        push_escaped_measurement(&mut escaped, value);
        escaped
    }

    #[inline]
    fn escape_string_field_value(value: &str) -> String {
        let mut escaped = String::with_capacity(value.len() + 2);
        push_escaped_string_field_value(&mut escaped, value);
        escaped
    }

    #[test]
    fn line_serialization_test() {
        let point = Point::new("test")
            .add_field("somefield", Value::Integer(65))
            .add_tag("sometag", Value::Boolean(false));
        let points = Points::new(point);

        assert_eq!(
            line_serialization(&points),
            "test,sometag=false somefield=65i\n"
        )
    }

    #[test]
    fn escape_keys_and_tags_test() {
        assert_eq!(
            escape_keys_and_tags("foo, hello=world"),
            "foo\\,\\ hello\\=world"
        )
    }

    #[test]
    fn escape_measurement_test() {
        assert_eq!(escape_measurement("foo, hello"), "foo\\,\\ hello")
    }

    #[test]
    fn escape_string_field_value_test() {
        assert_eq!(escape_string_field_value("\"foo"), "\"\\\"foo\"")
    }

    #[test]
    fn escape_string_field_value_escapes_backslashes() {
        let point = Point::new("test").add_field("path", r"C:\tmp");
        let points = Points::new(point);

        assert_eq!(line_serialization(&points), "test path=\"C:\\\\tmp\"\n");
    }

    #[test]
    fn line_serialization_preserves_point_order_when_consuming_points() {
        let first = Point::new("first").add_field("value", 1);
        let second = Point::new("second").add_field("value", 2);
        let points = Points::create_new(vec![first, second]);

        assert_eq!(
            line_serialization(points),
            "first value=1i\nsecond value=2i\n"
        );
    }

    #[test]
    fn quote_ident_test() {
        assert_eq!(quote_ident("root"), "\"root\"")
    }

    #[test]
    fn quote_literal_test() {
        assert_eq!(quote_literal("root"), "\'root\'")
    }
}
