//! Extra HTTP routes added by later versions (exports, code chunks, media).

use crate::app::App;
use std::sync::Arc;
use crate::web::Request;

pub fn handle_extra(_app: &Arc<App>, req: Request, _path: &str, _qs: &[(String, String)]) -> std::io::Result<()> {
    req.respond(crate::http::not_found())
}
