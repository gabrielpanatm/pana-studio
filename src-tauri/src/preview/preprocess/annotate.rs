mod index;
mod inject;
mod paths;
mod range;

#[cfg(test)]
mod tests;

pub use index::SourceIdIndex;
#[cfg(test)]
pub use inject::preprocess_template;
pub use inject::preprocess_template_with_revision;
pub use paths::is_template_relative_path;
