use anyhow::Result;
use serde_yaml_ng::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl std::fmt::Display for YamlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(col)) => write!(f, "Line {}, Col {}: {}", line, col, self.message),
            (Some(line), None) => write!(f, "Line {}: {}", line, self.message),
            _ => write!(f, "{}", self.message),
        }
    }
}

pub fn validate_yaml(text: &str) -> Result<Value, YamlError> {
    serde_yaml_ng::from_str(text).map_err(|e| {
        let msg = e.to_string();
        let (line, column) = parse_yaml_error_location(&msg);
        YamlError { message: msg, line, column }
    })
}

pub fn yaml_to_json(yaml_text: &str) -> Result<serde_json::Value, YamlError> {
    let yaml_value = validate_yaml(yaml_text)?;
    serde_json::to_value(&yaml_value).map_err(|e| YamlError {
        message: e.to_string(),
        line: None,
        column: None,
    })
}

pub fn json_to_yaml(json_value: &serde_json::Value) -> Result<String> {
    Ok(serde_yaml_ng::to_string(json_value)?)
}

fn parse_yaml_error_location(error_msg: &str) -> (Option<usize>, Option<usize>) {
    // serde_yaml_ng errors contain "at line X column Y"
    let mut line = None;
    let mut column = None;

    if let Some(line_idx) = error_msg.find("at line ") {
        let after_line = &error_msg[line_idx + 8..];
        if let Some(end) = after_line.find(|c: char| !c.is_ascii_digit()) {
            line = after_line[..end].parse().ok();
        }
    }

    if let Some(col_idx) = error_msg.find("column ") {
        let after_col = &error_msg[col_idx + 7..];
        if let Some(end) = after_col.find(|c: char| !c.is_ascii_digit()) {
            column = after_col[..end].parse().ok();
        } else {
            column = after_col.trim().parse().ok();
        }
    }

    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_yaml() {
        let yaml = "name: test\nversion: 1\nitems:\n  - a\n  - b\n";
        let result = validate_yaml(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_invalid_yaml() {
        let yaml = "key: [invalid\n  yaml: here";
        let result = validate_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_yaml_error_display() {
        let err =
            YamlError { message: "syntax error".to_string(), line: Some(5), column: Some(10) };
        assert_eq!(err.to_string(), "Line 5, Col 10: syntax error");
    }

    #[test]
    fn test_yaml_error_display_no_location() {
        let err = YamlError { message: "unknown error".to_string(), line: None, column: None };
        assert_eq!(err.to_string(), "unknown error");
    }

    #[test]
    fn test_yaml_to_json() {
        let yaml = "name: test\ncount: 42\n";
        let json = yaml_to_json(yaml).unwrap();
        assert_eq!(json["name"], "test");
        assert_eq!(json["count"], 42);
    }

    #[test]
    fn test_json_to_yaml() {
        let json = serde_json::json!({"name": "test", "count": 42});
        let yaml = json_to_yaml(&json).unwrap();
        assert!(yaml.contains("name: test"));
        assert!(yaml.contains("count: 42"));
    }

    #[test]
    fn test_empty_yaml() {
        let result = validate_yaml("");
        // Empty string is valid YAML (null)
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_k8s_resource_yaml() {
        let yaml = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nginx
  labels:
    app: nginx
spec:
  replicas: 3
  selector:
    matchLabels:
      app: nginx
  template:
    metadata:
      labels:
        app: nginx
    spec:
      containers:
      - name: nginx
        image: nginx:1.25
        ports:
        - containerPort: 80
"#;
        let result = validate_yaml(yaml);
        assert!(result.is_ok());
    }
}
