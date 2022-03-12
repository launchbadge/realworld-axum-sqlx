# realworld-axum-sqlx
A Rust implementation of the [Realworld] demo app spec showcasing the use of the [Axum] web framework and [SQLx]
SQL database client, with [PostgreSQL] as the database backend.

This project also serves as a commentary on the Realworld spec and how realistic it actually is, as well as
what a particular senior developer at Launchbadge currently considers to be best practices. Best practices are always
in flux, and a major point of this project was to experiment with project architecture and suss out what those
best practices might look like.

Feedback is appreciated!

### Note: "I" vs "we"

All comments in this project were (currently) written by a single person, and represent primarily that person's opinions
and observations. When a comment uses "we", it does not necessarily indicate an authoritative position taken by 
Launchbadge, but rather an interpretation, by that one person, of the current consensus at Launchbadge, 
or an observation of a particular sentiment shared by several developers at Launchbadge.

[Realworld]: https://github.com/gothinkster/realworld
[Axum]: https://github.com/tokio-rs/axum/
[SQLX]: https://github.com/launchbadge/sqlx/
[PostgreSQL]: https://www.postgresql.org/

## Project Structure

This project uses the 2015/1.0.0 module structure, with `mod.rs` files for modules with children,
as opposed to the 2018 (or "new") module style introduced in [RFC 2126], where `mod.rs` files are optional
and adding children to a module `foo.rs` is as simple as creating a `foo/` directory. 

This is a style choice we vacillated on for a while at Launchbadge. However, we ultimately decided that the 2018
module style results in a lot of papercuts during rapid development, which is antithetical to its original design.

Namely, because most file management GUIs sort files separately from folders, you have to jump between two completely
different places in the visualized file tree when transitioning between a parent module and its children. This is highly
confusing to developers who were already used to the original module system when the 2018 style was introduced.

On its own, this would have been manageable, but it is made worse by the fact that the new style isn't
recommended by default in the 2018 edition and there is no lint in `rustc` to enforce consistency*, so it becomes really 
easy to mix and match styles accidentally when multiple developers with varying experience levels are contributing to a 
project, or jumping back and forth between projects.

In retrospect, the 2018 edition should have involved a wholesale transition, banning `mod.rs` files by default and
providing a migration path with `cargo fix`, like with the other changes proposed in RFC 2126. 
I understand why the language team was hesitant to do this, as it had the weakest set of justifications of all
the changes proposed in the RFC, but their indecision resulted in, IMHO, a far, far worse situation.

\* Lints for this [were only recently added to Clippy][Clippy mod_module_files], 
several years after the 2018 style was introduced. As of writing, the `rust-lang/rust` repo itself mixes both
styles in various directories, which is frankly quite horrifying.

[RFC 2126]: https://github.com/rust-lang/rfcs/blob/master/text/2126-path-clarity.md#the-modrs-file
[Clippy mod_module_files]: https://github.com/rust-lang/rust-clippy/blob/master/clippy_lints/src/module_style.rs#L35

### Code Tour

If you're familiar with SQL, I recommend starting in the `migrations/` directory, as that's what contains the SQL files
that define the schema of the PostgreSQL database, which is usually the first thing done when prototyping a new project. 
The files there are filled with comments explaining the various decisions made while structuring the database,
as well as advice on good practices for schema architecture and how this compares to the Realworld spec.

Next, of course, is `main.rs` as the entrypoint for the application. It shows the typical boilerplate that goes
into spinning up a Rust backend application.

I then recommend going to `lib.rs` and recursively exploring the modules as they're defined.
Comments on the module definitions will guide you from there.

[`clap`]: https://github.com/clap-rs/clap/

## Setup

### Clone this Repository

```shell
$ git clone https://github.com/launchbadge/realworld-axum-sqlx
$ cd realworld-axum-sqlx
```

### Installing Rust and Cargo

Install Rust as described in [The Rust Programming Language, chapter 1](https://doc.rust-lang.org/book/ch01-01-installation.html).

This is the official Rust language manual and is freely available on doc.rust-lang.org.

The latest stable version is fine.


### Installing `sqlx-cli`

SQLx provides a command-line tool for creating and managing databases as well as migrations. It is published
on the Cargo crates registry as `sqlx-cli` and can be installed like so:

```shell
$ cargo install sqlx-cli --features postgres
```

### Running Postgres

By far the easiest way to run Postgres these days is using a container with [a pre-built image][docker-postgres].

The following command will start version 14 of Postgres (the latest at time of writing) using [Docker] 
(this command should also work with [Podman], a daemonless FOSS alternative).

```shell
$ docker run -d --name postgres-14 -p 5432:5432 -e POSTGRES_PASSWORD={password} postgres:14
```

Set `{password}` to a password of your choosing.

Ensure the Postgres server is running:
```shell
$ docker ps
```
```shell
CONTAINER ID   IMAGE         COMMAND                  CREATED          STATUS          PORTS                                       NAMES
621eb8962016   postgres:14   "docker-entrypoint.s…"   30 seconds ago   Up 30 seconds   0.0.0.0:5432->5432/tcp, :::5432->5432/tcp   postgres-14
```

[docker-postgres]: https://hub.docker.com/_/postgres
[Docker]: https://www.docker.com/
[Podman]: https://podman.io/

### Configuring the Application

Configuring the backend application is done, preferentially, via environment variables. This is the easiest way
to pass sensitive configuration data like database credentials and HMAC keys in a deployment environment such as 
[Kubernetes secrets].

To make working with environment variables easier during development, we can use [.env files] to avoid having
to define the variables every time.

As a starting point, you can simply `cp .env.sample .env` in this repo and modify the `.env` file as described by
the comments there.

[Kubernetes secrets]: https://kubernetes.io/docs/concepts/configuration/secret/
[.env files]: https://github.com/dotenv-rs/dotenv

### Setting Up the Application Database

With `sqlx-cli` installed and your `.env` file set up, you only need to run the following command to get the
Postgres database ready for use:

```
$ sqlx db setup
```

### Starting the Application

With everything else set up, all you should have to do at this point is:

```
$ cargo run
```

If successful, the Realworld-compatible API is now listening at port 8080.

## License

All code in this project is licensed under the [GNU Affero General Public License (AGPL)][AGPL]. 

The AGPL is an extension of the GPL which includes interacting with the application over a computer network in its 
definition of "distribution" for applying the license terms. If you modify this project and host it in a location that
is accessible to the web, you must make the source available as per the terms of the license.

See [LICENSE](LICENSE) in this repository for the text of the AGPL.

[AGPL]: https://www.gnu.org/licenses/agpl-3.0.en.html

### Contributing

Because enforcement of the AGPL requires that we own the copyright on the whole project, any contributions
to this project must be explicitly assigned copyright to Launchbadge, LLC. We're still researching the best
route to do this, so while we will allow PRs to be opened against this repository, they may not be merged right away.
