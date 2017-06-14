use ::{Points, Value};

/// Resolve the points to line protocol format
pub fn line_serialization(points: Points) -> String {
    let mut line = Vec::new();
    for point in points.point {
        line.push(point.measurement);

        for (tag, value) in point.tags.iter() {
            line.push(",".to_string());
            line.push(tag.to_string());
            line.push("=".to_string());

            match value {
                &Value::String(ref s) => line.push(s.to_string()),
                &Value::Float(ref f) => line.push(f.to_string()),
                &Value::Integer(ref i) => line.push(i.to_string()),
                &Value::Boolean(b) => line.push({ if b { "true".to_string() } else { "false".to_string() } })
            }
        }

        let mut was_first = true;

        for (field, value) in point.fields.iter() {
            line.push({
                if was_first {
                    was_first = false;
                    " "
                } else { "," }
            }.to_string());
            line.push(field.to_string());
            line.push("=".to_string());

            match value {
                &Value::String(ref s) => line.push(s.to_string()),
                &Value::Float(ref f) => line.push(f.to_string()),
                &Value::Integer(ref i) => line.push(i.to_string()),
                &Value::Boolean(b) => line.push({ if b { "true".to_string() } else { "false".to_string() } })
            }
        }

        match point.timestamp {
            Some(t) => {
                line.push(" ".to_string());
                line.push(t.to_string());
            }
            _ => {}
        }

        line.push("\n".to_string())
    }

    line.join("")
}
