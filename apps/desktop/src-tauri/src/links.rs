//! Links on the About page, in one place for both apps.
use serde_json::{json, Value};

// TODO(links): confirm these URLs. The app config has no website or guide URL; the website
// follows the bundle identifier (app.getdoze.desktop) and the guide path is a placeholder.
const WEBSITE: &str = "https://getdoze.app";
const MCP_GUIDE: &str = "https://getdoze.app/mcp";

pub fn json() -> Value {
    json!([
        { "title": "getdoze.app", "url": WEBSITE },
        { "title": "MCP guide", "url": MCP_GUIDE },
    ])
}
