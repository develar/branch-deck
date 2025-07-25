# Changelog

## [0.6.0](https://github.com/develar/branch-deck/compare/v0.5.0...v0.6.0) (2025-07-16)


### Features

* unassigned commits ([52a9c7d](https://github.com/develar/branch-deck/commit/52a9c7da384cc30a5befa7820581024eb362daa0))

## [0.5.0](https://github.com/develar/branch-deck/compare/v0.4.2...v0.5.0) (2025-07-14)


### Features

* merge conflict viewer ([3ce1b34](https://github.com/develar/branch-deck/commit/3ce1b34dd537f338f6fb67d143e93dc43025e69e))
* merge conflict viewer (fix sub windows) ([fdc13b3](https://github.com/develar/branch-deck/commit/fdc13b3dc5954d310944dcf59a2b2bb69fc102c2))

## [0.4.2](https://github.com/develar/branch-deck/compare/v0.4.1...v0.4.2) (2025-07-08)


### Bug Fixes

* case-insensitive git config ([31e39d4](https://github.com/develar/branch-deck/commit/31e39d430af171be3c02f63502f11093ebaf12c3))
* move auto-update logic from js to rust to improve security ([b937316](https://github.com/develar/branch-deck/commit/b937316ee19729f90bbe8209de63d6bba40f73fe))
* use `()` instead of `[]` as prefix because `[]` is a common convention for subsystems ([8a78518](https://github.com/develar/branch-deck/commit/8a78518cb4a045b882d8459c1b09729c09b6c004))

## [0.4.1](https://github.com/develar/branch-deck/compare/v0.4.0...v0.4.1) (2025-07-07)


### Bug Fixes

* relaunch after auto-update ([da5ec34](https://github.com/develar/branch-deck/commit/da5ec34bcde63b7a8eb774ad2dfd9d73e6a139da))


### Performance Improvements

* merge using tree, no need to use commits ([4ba8048](https://github.com/develar/branch-deck/commit/4ba8048cca2d1797380f50ece1885b6f14600a4e))

## [0.4.0](https://github.com/develar/branch-deck/compare/branch-deck-v0.3.0...branch-deck-v0.4.0) (2025-07-06)


### Features

* branch parallel processing, sanitize branch name ([8821954](https://github.com/develar/branch-deck/commit/8821954bae9f1aad67c72879af588dce6bd9894e))


### Bug Fixes

* slow cherry-pick ([252f076](https://github.com/develar/branch-deck/commit/252f07683fd0bd7ed29b520a7324f7de1e8992f9))
* truncate commit message text ([a9bcecf](https://github.com/develar/branch-deck/commit/a9bcecf4c9e789e00a9d96e532cd5d5ae6e8c37e))
* use a correct base commit to avoid cherry-picking unrelated changes ([372ebe7](https://github.com/develar/branch-deck/commit/372ebe7bc4a6e4f03d9da0bd18a3d7efa3ede640))

## [0.3.0](https://github.com/develar/branch-deck/compare/branch-deck-v0.2.0...branch-deck-v0.3.0) (2025-07-06)


### Features

* auto-update ([00ab5ef](https://github.com/develar/branch-deck/commit/00ab5efc56384520a83678c5264ea5682ec95659))


### Bug Fixes

* truncate commit message text ([a9bcecf](https://github.com/develar/branch-deck/commit/a9bcecf4c9e789e00a9d96e532cd5d5ae6e8c37e))

## [0.2.0](https://github.com/develar/branch-deck/compare/branch-deck-v0.1.0...branch-deck-v0.2.0) (2025-07-02)


### Features

* branch parallel processing, sanitize branch name ([8821954](https://github.com/develar/branch-deck/commit/8821954bae9f1aad67c72879af588dce6bd9894e))


### Bug Fixes

* use a correct base commit to avoid cherry-picking unrelated changes ([372ebe7](https://github.com/develar/branch-deck/commit/372ebe7bc4a6e4f03d9da0bd18a3d7efa3ede640))
