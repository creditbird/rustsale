use md5::{Digest, Md5};
use serde_json::Value;

/// Generates the MD5 signature signature key for a request type and parameter object.
pub fn get_sign_key(request_type: &str, params: &Value) -> String {
    let mut sign_str = format!("zsecure{}", request_type);
    if let Some(obj) = params.as_object() {
        let mut keys: Vec<&String> = obj.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(val) = obj.get(key) {
                let val_str = match val {
                    Value::Null => "".to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(num) => num.to_string(),
                    Value::String(s) => s.clone(),
                    _ => val.to_string(),
                };
                sign_str.push_str(&val_str);
            }
        }
    }
    let mut hasher = Md5::new();
    hasher.update(sign_str.as_bytes());
    format!("{:x}", hasher.finalize())
}
