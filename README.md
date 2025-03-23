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

- `ALU_DATABASE_URL`: The URL of the SurrealDB instance to use. If not set, Alumulemu will use the RocksDB backend mounted at `/data` in the container, or the `database` directory in the working directory if running from source. (`rocksdb:///data` or `rocksdb://database` respectively)
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

### Running

You can run a Docker/Podman container with the provided example `docker-compose.yml` file.

You may also build this project from source.

### Building and developing

Alumulemu is built using Rust and Vue.js.

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
