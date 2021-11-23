/// The configuration parameters for the application.
///
/// See `.env.sample` in the repository for details.
#[derive(clap::Parser)]
pub struct Config {
    /// The connection URL for the Postgres database this application should use.
    #[clap(long, env)]
    pub database_url: String,

    /// The HMAC signing and verification key used for login tokens (JWTs).
    #[clap(long, env)]
    pub hmac_key: String,
}