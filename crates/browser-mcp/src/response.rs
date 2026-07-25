// @file crates/browser-mcp/src/response.rs
// @description Content origin tagging for all MCP tool responses.
// @layer mcp
// @created meerita <meerita@icloud.com>

use serde_json::{json, Value};

// Every response carrying web content must include these fields so MCP clients
// can never treat remote content as trusted instructions.
pub(crate) fn tagged_response(url: &str, payload: Value) -> Value {
    let mut base = json!({
        "content_origin": "remote_web_page",
        "trusted": false,
        "url": url,
    });
    if let (Some(base_obj), Value::Object(extra)) = (base.as_object_mut(), payload) {
        for (k, v) in extra {
            base_obj.insert(k, v);
        }
    }
    base
}
