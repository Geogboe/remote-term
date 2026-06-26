pub const INDEX_HTML: &str = include_str!("static/index.html");
pub const MAIN_JS: &str = include_str!("static/main.js");
pub const STYLE_CSS: &str = include_str!("static/style.css");

pub fn content_type(path: &str) -> &'static str {
    match path {
        "main.js" => "text/javascript; charset=utf-8",
        "style.css" => "text/css; charset=utf-8",
        _ => "text/html; charset=utf-8",
    }
}
