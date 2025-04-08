![Sablier Banner](/assets/banner-merkle-api.png)

# Sablier MerkleAPI [![Github Actions][gha-badge]][gha] [![License: LGPL v3][license-badge]][license] [![Sablier][twitter-badge]][twitter]

[gha]: ../../actions
[gha-badge]: ../../actions/workflows/ci.yml/badge.svg
[license]: https://www.gnu.org/licenses/gpl-3.0
[license-badge]: https://img.shields.io/badge/License-GPL_v3-blue.svg
[twitter]: https://twitter.com/Sablier
[twitter-badge]: https://img.shields.io/twitter/follow/Sablier?label=%40Sablier

A Rust-based API for generating and verifying Merkle trees used in Sablier.

For more details about Sablier, check out our [website](https://sablier.com) and our documentation at
[docs.sablier.com](https://docs.sablier.com/api/airdrops/merkle-api/overview).

## About

[Sablier Airdrops](https://app.sablier.com/airdrops) rely on pre-configured Merkle trees. This data structure contains
the list of recipients as well as their individual claim details. Utilities are required to create, manage and validate
such Merkle trees.

To make these functionalities available to the Sablier client interfaces as well as 3rd party integrators we've created
a Rust backend service called `merkle-api`. Through a REST API, it provides access to creating, storing and reading from
Airstream related Merkle trees.

## Development

To properly integrate the Sablier Merkle API into your own product or perform local tests, please consult the docs at
[docs.sablier.com](https://docs.sablier.com/api/drops/merkle-api/overview).

### API

The API provides endpoints that support actions like: creating a campaign, checking eligibility for a particular address
etc. For more details, see the [endpoints docs](https://docs.sablier.com/api/merkle-api/functionality).

### CSV

You can see an example of the Rust CSV Generator here:

https://gist.github.com/gavriliumircea/2a9797f207a2a2f3832ddaa376337e8c

All the rules for formatting such CSV are explained [here](https://docs.sablier.com/apps/guides/csv-support).

### Contributing

Feel free to dive in! [Open](../../issues/new) an issue, [start](../../discussions/new) a discussion or submit a PR.

#### Pre Requisites

You will need the following software on your machine:

- [Git](https://git-scm.com/downloads)
- [Rust](https://rust-lang.org/tools/install)
- [Cargo](https://doc.rust-lang.org/cargo/commands/cargo-install.html)

#### Syntax Highlighting

You will need the following VSCode extensions:

- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
- [rust-syntax](https://marketplace.visualstudio.com/items?itemName=dustypomerleau.rust-syntax)
- [prettier](https://marketplace.visualstudio.com/items?itemName=esbenp.prettier-vscode)

#### Recommendations

We recommend forking this repository and running the Merkle backend using either your own infrastructure or a Vercel
project hosted under an account you own. This guarantees you'll have more control over the uptime of the service, as
well as access to add any custom features or optimizations you may need.

## License

Sablier Merkle API is licensed under [GPL v3 or later](./LICENSE.md).
