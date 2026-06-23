pub fn server_name() -> &'static str {
    "loom-mcp-server"
}

pub fn smoke_message() -> String {
    format!("{} {}", server_name(), delivery_core::VERSION)
}
