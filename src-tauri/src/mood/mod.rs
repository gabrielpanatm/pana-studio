mod resource_budget;
mod storage;

pub use storage::{
    export_mood_board_binary_asset, export_mood_board_svg_asset,
    normalize_mood_board_image_relative_path, normalize_mood_board_svg_source_relative_path,
    read_mood_board, write_mood_board,
};

pub(crate) use resource_budget::acquire_heavy_mood_asset_operation;
pub(crate) use storage::{
    extract_mood_board_image_palette_with_reader, read_mood_board_image_data_url_with_reader,
    read_mood_board_image_original_data_url_with_reader, MAX_MOOD_BOARD_ASSET_BYTES,
    MAX_MOOD_BOARD_SVG_BYTES,
};
