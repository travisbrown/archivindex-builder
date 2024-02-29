# Archivindex Builder

[![Rust build status](https://img.shields.io/github/actions/workflow/status/travisbrown/archivindex-builder/ci.yaml?branch=main)](https://github.com/travisbrown/archivindex-builder/actions)
[![Coverage status](https://img.shields.io/codecov/c/github/travisbrown/archivindex-builder/main.svg)](https://codecov.io/github/travisbrown/archivindex-builder)

This is the primary repository for the Archivindex Builder project, which has been supported by [Prototype Fund][prototype-fund].

## About

![Screenshot of the Archivindex Builder search interface](images/search-example-01.png)

## Packages

* [`aib-core`](core/): Representations of entries, snapshots, etc.
* [`aib-cdx`](cdx/): Client for accessing [CDX][cdx] index APIs
* [`aib-cdx-store`](cdx-store/): Local store for CDX index data
* [`aib-store`](store/): Local store for archive snapshots
* [`aib-downloader`](downloader/): Client for downloading archive snapshots
* [`aib-downloader-cli`](downloader-cli/): Minimal command-line interface for downloading archive snapshots (for use in environments where compiling the entire project is undesirable)
* [`aib-extractor`](extractor/): Library for extracting indexable documents from snapshots
* [`aib-indexer`](indexer/): Full-text index for snapshots, built on [Tantivy][tantivy]
* [`aib-manager`](manager/): Pipelines for operations involving multiple data sources
* [`aib-cli`](cli/): Command-line interfaces for management operations
* [`aib-auth`](auth/): Interfaces for managing user authentication 
* [`aib-auth-sqlx`](auth-sqlx/): [SQLx][sqlx] implementation for managing user authentication 
* [`aib-service`](service/): JSON web service providing search API, built on [Rocket][rocket]
* [`redirects`](redirects/): Miscellaneous tools for working with Wayback Machine redirects

[cdx]: https://www.loc.gov/preservation/digital/formats/fdd/fdd000590.shtml
[prototype-fund]: https://prototypefund.de
[rocket]: https://rocket.rs
[sqlx]: https://github.com/launchbadge/sqlx
[tantivy]: https://github.com/quickwit-oss/tantivy