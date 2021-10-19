// lint me harder
#![forbid(non_ascii_idents)]
#![forbid(unsafe_code)]
#![deny(keyword_idents)]
#![deny(elided_lifetimes_in_paths)]
#![deny(rust_2021_incompatible_closure_captures)]
#![deny(rust_2021_incompatible_or_patterns)]
#![deny(rust_2021_prefixes_incompatible_syntax)]
#![deny(unused_crate_dependencies)]
#![warn(explicit_outlives_requirements)]
#![warn(macro_use_extern_crate)]
#![warn(meta_variable_misuse)]
#![warn(noop_method_call)]
#![warn(pointer_structural_match)]
#![warn(rust_2021_prelude_collisions)]
#![warn(semicolon_in_expressions_from_macros)]
#![warn(single_use_lifetimes)]
#![warn(trivial_numeric_casts)]
#![warn(unused_lifetimes)]
#![warn(unused_qualifications)]
#![warn(variant_size_differences)]
#![deny(clippy::fallible_impl_from)]
#![deny(clippy::wildcard_dependencies)]
#![warn(clippy::pedantic)]
#![warn(clippy::clone_on_ref_ptr)]
#![warn(clippy::create_dir)]
#![warn(clippy::debug_assert_with_mut_call)]
#![warn(clippy::decimal_literal_representation)]
#![warn(clippy::empty_line_after_outer_attr)]
#![warn(clippy::exit)]
#![warn(clippy::get_unwrap)]
#![warn(clippy::indexing_slicing)]
#![warn(clippy::lossy_float_literal)]
#![warn(clippy::option_if_let_else)]
#![warn(clippy::panic)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::rc_mutex)]
#![warn(clippy::redundant_field_names)]
#![warn(clippy::rest_pat_in_fully_bound_structs)]
#![warn(clippy::shadow_unrelated)]
#![warn(clippy::str_to_string)]
#![warn(clippy::string_lit_as_bytes)]
#![warn(clippy::string_to_string)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::useless_let_if_seq)]
#![allow(clippy::missing_errors_doc)]

use tracing_subscriber::EnvFilter;

pub mod server;
pub mod socket_traits;
pub mod types;
pub mod ui;

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    server::Server::run_tcp("127.0.0.1:5555")?;
    Ok(())
}
