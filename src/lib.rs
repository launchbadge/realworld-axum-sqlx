// The normal style for documenting modules is to place the doc-comments inside the module
// files at the top with `//!`, known as internal doc comments.
//
// However, this style better facilitates a guided exploration of the code, so it's the one
// we'll be using in this project.

/// Defines the arguments required to start the server application using [`clap`].
///
/// [`clap`]: https://github.com/clap-rs/clap/
pub mod config;

/// Contains the setup code for the API build with Axum.
///
/// The Realworld API routes exist in child modules of this.
pub mod http;
