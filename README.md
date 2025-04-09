# Alumulemu - A [Horizon] package repository manager

**Alumulemu** is a package repository manager for the [Horizon] operating system (AKA the Nintendo Switch OS). It's designed to be relatively simple to set up, use and maintain. Powered by [SurrealDB](https://surrealdb.com/) and [Axum](https://github.com/tokio-rs/axum).

## Overview

Alumulemu is a package repository manager for the Nintendo Switch, designed to be simple to set up, use, and maintain. It allows you to host your own Tinfoil repository for consumption by Tinfoil (or any other client that supports the Tinfoil JSON index format).

Alumulemu exports the repository as a [Tinfoil index](https://blawar.github.io/tinfoil/custom_index), so it can be consumed by [Tinfoil](https://tinfoil.io/).

> [!NOTE]
> Alumulemu is still in development and is not yet ready for production use.
>
> Plans to also write a standalone client are also in the works, so you can consume Tinfoil/Alumulemu repositories without Tinfoil.
>
> Contributions and feedback are welcome to help improve the development process and enhance features.

## Features

- **SurrealDB-based backend**: Alumulemu uses [SurrealDB](https://surrealdb.com/) for efficient data storage and retrieval, allowing for the database to be easily scaled, managed, and queried. You may run the repository from an embedded instance or link it to an external SurrealDB instance.
- Web interface: Alumulemu provides a web interface for managing the server and viewing package metadata. The web interface is built using [Vue.js](https://vuejs.org/) and [Tailwind CSS](https://tailwindcss.com/).
- **REST API**: Alumulemu provides a REST API for managing the repository and querying package metadata. Documentation for the API will be available in the future.

## Planned features

- Merging/proxying other Tinfoil repositories, allowing alumulemu to act as a central repository for multiple upstream repositories. (PARTIALLY DONE)
  - Merging with non-HTTP or file-based sources, such as Google Drive, 1fichier, or other sources.
  - Caching upstream repositories for faster access
  - ~~Merging metadata from other Tinfoil repositories/alumulemu instances~~ (DONE, Indexes are now fetched and merged into the main index every 6 hours)
- Encryption support, as seen in [Tinfoil DRM specification](https://blawar.github.io/tinfoil/drm/).
- Custom metadata editing and management by title ID
  - Interface to edit metadata for a specific title ID (You can already do this by merging with another Tinfoil index, but this is not user-friendly)
- Tinfoil Theme repository support
  - Blacklist/whitelist for specific themes
  - Optional MOTD support (Currently disabled explicitly in alumulemu)

## "Alumulemu"? What??

The name "Alumulemu" is a mispronunciation of "Aluminium", spoken with a thick Chinese accent. It initially stemmed from a series of ads for a [Chinese
prefabricated house company](https://www.etonghouse.com/), where the saleswoman would advertise prefabricated homes in a very thick Chinese accent and broken English. The name is also in contrast to the name "Tinfoil" since Aluminum foil is a more common material than Tin.

[Horizon]: https://en.wikipedia.org/wiki/Nintendo_Switch_system_software

## Usage

You will require:

- A Nintendo Switch or Switch 2 console (optional, jailbroken)
- A server to host Alumulemu
- SurrealDB instance (optional, recommended for better performance)
- Console-specific cryptographic keys (required, obtain from your own device or take someone else's)
- A stable network connection (optional)
- Your own game dumps (optional)

### Setup

1. Dump the keys from your console using [Lockpick_RCM](https://github.com/saneki/Lockpick_RCM).
2. (Optional) Set up a SurrealDB instance. You can use the [official SurrealDB Docker image](https://hub.docker.com/r/surrealdb/surrealdb) somewhere or use the RocksDB backend included with Alumulemu.
3. Pull the OCI image from the [GitHub Container Registry](https://github.com/RyouVC/alumulemu/packages/).
4. Run the container with the required environment variables and volumes.

### Configuration

Alumulemu is configured using environment variables. The following environment variables are required:

- `ALU_DATABASE_URL`: The URL of the SurrealDB instance to use. If not set, Alumulemu will use the RocksDB backend mounted at `/data` in the container, or the `database` directory in the working directory if running from source. (`surrealkv:///data` or `surrealkv://database` respectively)
  - `ALU_DATABASE_AUTH_METHOD`: The authentication method for the SurrealDB instance (optional). By default it will assume no authentication is required, used for embedded instances. Available options are `none`, `root` (todo: implement namespace auth).
    - `ALU_SURREAL_ROOT_USERNAME`: The username to use for the root user (optional). Required if `ALU_DATABASE_AUTH_METHOD` is set to `root`.
    - `ALU_SURREAL_ROOT_PASSWORD`: The password to use for the root user (optional). Required if `ALU_DATABASE_AUTH_METHOD` is set to `root`.
  - `ALU_SURREAL_NAMESPACE`: The namespace to use for the database (optional). If not set, Alumulemu will use the default namespace. (`alumulemu`)
  - `ALU_SURREAL_DATABASE`: The database to use for the namespace (optional). If not set, Alumulemu will use the default database. (`alumulemu`)

- `ALU_PRIMARY_REGION` (optional): Primary eShop metadata region to use. Defaults to `US`.
- `ALU_PRIMARY_LANGUAGE` (optional): Primary eShop metadata language to use. Defaults to `en`.

The region and language code is combined to form the locale code used to query the eShop title database. You may find the list of supported locales [here](https://github.com/blawar/titledb/blob/master/languages.json).

- `ALU_SECONDARY_LOCALES` (optional): Secondary eShop metadata locales to pull from. Defaults to blank (no secondary locales). Values are comma-separated locale codes, delimited by an underscore. For example, `JP_ja,US_es` will pull Japanese titles from the Japanese eShop and Spanish titles from the US eShop.

- `ALU_PROD_KEYS`: The path to the Switch production keys file. This is required to decrypt data from your ROMs.
- `ALU_TITLE_KEYS`: The path to the Switch title keys file. This is required to decrypt some titles and DLCs.

- `ALU_HOST`: The host to bind the server to. Defaults to `0.0.0.0:3000`.
- `ALU_CACHE_DIR`: The directory to cache title database files in. Defaults to `.` (current working directory) or `/var/cache/alumulemu` if running in a container.
- `ALU_PUBLIC`: Whether to run the server in public mode. Defaults to `false`. If set to `true`, the server will not require authentication to access the API. However administrative endpoints will still require authentication if there are users in the database.

#### Optimizing database performance

You may switch to a different SurrealDB backend for better performance. The following backends are available for SurrealDB:

- `surrealkv`: The default backend, using [SurrealKV](https://surrealdb.com/docs/surrealkv) for key-value storage. This is the fastest local backend, but may be less reliable. It's set as default due to its speed.
- `rocksdb`: The RocksDB backend, using [RocksDB](https://rocksdb.org/) for key-value storage. This is the most reliable backend, but may be slower than SurrealKV. It's recommended to use this backend for production environments.
- `tikv`: The TiKV backend, using [TiKV](https://tikv.org/) for distributed key-value storage. This is the most scalable backend, but may be slower depending on your network configuration. It's recommended to use this backend for large-scale deployments, such as multi-node clusters.

You may also connect to an external SurrealDB instance using WebSockets.

To set the database path, set the `ALU_DATABASE_URL` environment variable to the appropriate URL. For example, to use the RocksDB backend, set `ALU_DATABASE_URL` to `rocksdb:///data`, `surrealkv:///data` for SurrealKV, or `ws://localhost:8000` for an external instance.

### Access titles

Alumulemu provides a web interface for viewing title metadata. You can simply go to the URL of your server in a web browser to access the interface.

#### Using Tinfoil

Alumulemu also provides a Tinfoil-compatible JSON index for use with Tinfoil. You can add the following URL to Tinfoil to access the repository:

```txt
http://<your-server-ip>:3000/api/tinfoil
```

### Running

You can run a Docker/Podman container with the provided example `docker-compose.yml` file.

You may also build this project from source.

> [!NOTE]
> Once the server is running, authentication is disabled by default when there are no users in the database. You should create a user by going to `/admin/users` and creating a user. It will then automatically lock down the server to require authentication.
>
> It is **strongly recommended** to set up authentication before running the server in a public environment.

### Building and developing

Alumulemu is built using:

- Rust for the backend, with the following libraries as its core system:
  - [Tokio](https://tokio.rs/) for async I/O
  - [Axum](https://docs.rs/axum/latest/axum/) for the web server
  - [SurrealDB](https://surrealdb.com/) as the database backend, supporting both embedded and external instances
  - [nx-archive](https://github.com/RyouVC/nx-archive) for reading and parsing Nintendo Switch archives
  - [clap](https://github.com/clap-rs/clap) for configuration through environment variables (Although you can also pass them as positional arguments if you really have to)
  - [struson](https://github.com/Marcono1234/struson) for the streaming JSON parser, used for importing the eShop dataset from <https://github.com/blawar/titledb>

- Vue.js for the web UI, with the following frameworks:
  - [Tailwind CSS](https://tailwindcss.com/) for styling
  - [Vue Router](https://router.vuejs.org/) for routing
  - [DaisyUI](https://daisyui.com/) for additional styling

You will require the following tools to build Alumulemu:

- Rust toolchain (stable, MSRV 1.85 or later)
- Node.js (LTS)
- PNPM

To build the project, Simply run:

```sh
cargo build --release
```

This will execute the necessary build steps for both the backend and frontend. See [build.rs](build.rs) for more information.

### License

Alumulemu is licensed under the GNU Affero General Public License v3.0. See [LICENSE](LICENSE) for more information.

It is provided as-is with no warranty or guarantee of support. Use at your own risk.

Alumulemu is not affiliated with or endorsed by Nintendo, Team Xecuter, SurrealDB, or any other entity mentioned in this document.


[def]: https://github.com/blawar/titledb