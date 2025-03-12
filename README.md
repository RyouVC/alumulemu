# Alumulemu - A [Horizon] package repository manager

**Alumulemu** is a package repository manager for the [Horizon] operating system (AKA the Nintendo Switch OS). It's designed to be relatively simple to set up, use and maintain.

Alumulemu exports the repository as a [Tinfoil index](https://blawar.github.io/tinfoil/custom_index), so it can be consumed by [Tinfoil](https://tinfoil.io/).

> [!NOTE]
> Alumulemu is still in development and is not yet ready for production use.
>
> Plans to also write a standalone client are also in the works, so you can consume Tinfoil/Alumulemu repositories without Tinfoil.
>
> Contributions and feedback are welcome to help improve the development process and enhance features.

## Features

- **SurrealDB-based backend**: Alumulemu uses [SurrealDB](https://surrealdb.com/) for efficient data storage and retrieval, allowing for the database to be easily scaled, managed, and queried. You may run the repository from an embedded instance or link it to an external SurrealDB instance.

## "Alumulemu"? What??

The name "Alumulemu" is a mispronunciation of "Aluminium", spoken with a thick Chinese accent. It initially stemmed from a series of ads for a [Chinese
prefabricated house company](https://www.etonghouse.com/), where the saleswoman would advertise prefabricated homes in a very thick Chinese accent and broken English. The name is also in contrast to the name "Tinfoil" since Aluminum foil is a more common material than Tin.

[Horizon]: https://en.wikipedia.org/wiki/Nintendo_Switch_system_software
