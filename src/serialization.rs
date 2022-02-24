use crate::{Point, Value};
use std::borrow::Borrow;

/// Resolve the points to line protocol format
pub(crate) fn line_serialization<'a>(
    points: impl IntoIterator<Item = impl Borrow<Point<'a>>>,
) -> String {
    let mut line = String::new();

    for point in points {
        let point: &Point = point.borrow();
        line.push_str(&escape_measurement(&point.measurement));

        for (tag, value) in &point.tags {
            line.push(',');
            line.push_str(&escape_keys_and_tags(tag));
            line.push('=');

            match value {
                Value::String(s) => line.push_str(&escape_keys_and_tags(s)),
                Value::Float(f) => line.push_str(f.to_string().as_str()),
                Value::Integer(i) => line.push_str(i.to_string().as_str()),
                Value::Boolean(b) => line.push_str({
                    if *b {
                        "true"
                    } else {
                        "false"
                    }
                }),
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
            line.push_str(&escape_keys_and_tags(field));
            line.push('=');

            match value {
                Value::String(s) => {
                    line.push_str(&escape_string_field_value(&s.replace("\\\"", "\\\\\"")))
                }
                Value::Float(f) => line.push_str(&f.to_string()),
                Value::Integer(i) => line.push_str(&format!("{i}i")),
                Value::Boolean(b) => line.push_str({
                    if *b {
                        "true"
                    } else {
                        "false"
                    }
                }),
            }
        }

        if let Some(t) = point.timestamp {
            line.push(' ');
            line.push_str(&t.to_string());
        }

        line.push('\n')
    }

    line
}

#[inline]
pub(crate) fn quote_ident(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('\"', "\\\"")
            .replace('\n', "\\n")
    )
}

#[inline]
pub(crate) fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

#[inline]
pub(crate) fn conversion(value: &str) -> String {
    value
        .replace('\'', "")
        .replace('\"', "")
        .replace('\\', "")
        .trim()
        .to_string()
}

#[inline]
fn escape_keys_and_tags(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

#[inline]
fn escape_measurement(value: &str) -> String {
    value.replace(',', "\\,").replace(' ', "\\ ")
}

#[inline]
fn escape_string_field_value(value: &str) -> String {
    format!("\"{}\"", value.replace('\"', "\\\""))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Point, Points};

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
    fn quote_ident_test() {
        assert_eq!(quote_ident("root"), "\"root\"")
    }

    #[test]
    fn quote_literal_test() {
        assert_eq!(quote_literal("root"), "\'root\'")
    }
}
