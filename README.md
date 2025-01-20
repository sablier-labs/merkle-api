![Sablier](/assets/banner-merkle-api.png)

# Sablier SDK: Merkle API

A rust based API for generating and verifying Merkle trees used in Sablier.

For more details about Sablier, check out our [website](https://sablier.com) and our documentation at
[docs.sablier.com](https://docs.sablier.com/api/drops/merkle-api/overview).

## About

Sablier Drops rely on pre-configured merkle trees. This data structure contains the list of recipients as well as their
individual claim details. Utilities are required to create, manage and validate such merkle trees.

To make these functionalities available to the Sablier client interfaces as well as 3rd party integrators we've created
a Rust backend service called `merkle-api`. Through a REST API, it provides access to creating, storing and reading from
Airstream related Merkle trees.

## Development

To properly integrate the Sablier V2 Merkle API into your own product or perform local tests, please consult
[this documentation](https://docs.sablier.com/api/drops/merkle-api/overview).

### API

The API provides endpoints that support actions like: creating a campaign, checking eligibility for a particular address
etc. For more details see [endpoints](https://docs.sablier.com/api/merkle-api/functionality)

### CSV

You can an example of a Rust CSV Generator here:

https://gist.github.com/gavriliumircea/2a9797f207a2a2f3832ddaa376337e8c

We've explained the rules for formatting such CSVs in our [docs](https://docs.sablier.com/apps/guides/csv-support).

### Contributing

Feel free to dive in! [Open](https://github.com/sablier-labs/v2-merkle-api/issues/new) an issue,
[start](https://github.com/sablier-labs/v2-merkle-api/discussions/new) a discussion or submit a PR.

#### Pre Requisites

You will need the following software on your machine:

- [Git](https://git-scm.com/downloads)
- [Rust](https://rust-lang.org/tools/install)
- [Cargo](https://doc.rust-lang.org/cargo/commands/cargo-install.html)

#### Syntax Highlighting

You will need the following VSCode extensions:

- [rust-syntax](https://marketplace.visualstudio.com/items?itemName=dustypomerleau.rust-syntax)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
- [prettier](https://marketplace.visualstudio.com/items?itemName=esbenp.prettier-vscode)

### Recommendations

We recommend forking this repository and running the merkle backend using your own infrastructure or a vercel
environment hosted under an account you own. This guarantees you'll have more control over the up-time of the service,
as well as access to add any custom features or optimizations you may require.

## License

Sablier V2 Merkle API is licensed under [GPL v3 or later](./LICENSE.md).
