![Sablier Banner](/assets/banner-merkle-api.png)

# Sablier MerkleAPI [![Github Actions][gha-badge]][gha] [![License: LGPL v3][license-badge]][license] [![Sablier][twitter-badge]][twitter]

[gha]: ../../actions
[gha-badge]: ../../actions/workflows/ci.yml/badge.svg
[license]: https://www.gnu.org/licenses/gpl-3.0
[license-badge]: https://img.shields.io/badge/License-GPL_v3-blue.svg
[twitter]: https://twitter.com/Sablier
[twitter-badge]: https://img.shields.io/twitter/follow/Sablier?label=%40Sablier

A Rust-based API for generating and verifying Merkle trees used in Sablier.

> [!IMPORTANT]
> This is a **private internal service** operated by Sablier Labs. It powers the official Sablier client
> interfaces and is not intended to be self-hosted or run by third parties. The source is published under GPL
> v3 for transparency and auditability only.
>
> If you are a third-party integrator and need programmatic access, consume the hosted API documented at
> [docs.sablier.com](https://docs.sablier.com/api/airdrops/merkle-api/overview). If you require a self-managed
> deployment, fork the repository and operate it under your own infrastructure — Sablier Labs does not provide
> support for external deployments.

## About

[Sablier Airdrops](https://app.sablier.com/airdrops) rely on pre-configured Merkle trees. This data structure contains
the list of recipients as well as their individual claim details. Utilities are required to create, manage and validate
such Merkle trees.

To make these functionalities available to the Sablier client interfaces we built `merkle-api`, a Rust backend service
that exposes a REST API for creating, storing and reading Airstream-related Merkle trees.

## API

The API provides endpoints for actions such as creating a campaign, checking eligibility for a given address, and
verifying proofs. See the [endpoints docs](https://docs.sablier.com/api/merkle-api/functionality) for the full
reference.

## CSV

An example of the Rust CSV generator is available here:

https://gist.github.com/gavriliumircea/2a9797f207a2a2f3832ddaa376337e8c

The formatting rules for input CSVs are documented [here](https://docs.sablier.com/apps/guides/csv-support).

## Contributing

Feel free to dive in! [Open](../../issues/new) an issue, [start](../../discussions/new) a discussion or submit a PR.

### Pre Requisites

You will need the following software on your machine:

- [Git](https://git-scm.com/downloads)
- [Rust](https://rust-lang.org/tools/install)
- [Cargo](https://doc.rust-lang.org/cargo/commands/cargo-install.html)

## License

Sablier Merkle API is licensed under [GPL v3 or later](./LICENSE.md).
