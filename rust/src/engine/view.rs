//! Map view pipeline entry point. The real refresh_map_view (orig 1000:5910) lives in
//! engine/rest.rs together with the rest of the view pipeline; this delegate keeps the
//! existing `view::refresh_map_view` callers working.
use crate::game::Game;
pub fn refresh_map_view(gm: &mut Game) { crate::engine::rest::refresh_map_view(gm); }
