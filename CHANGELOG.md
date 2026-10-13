# Changelog

## [0.11.0](https://github.com/develar/branch-deck/compare/v0.10.0...v0.11.0) (2026-10-13)


### Features

* commit preview opened with Space instead of the inline body ([dd4ec04](https://github.com/develar/branch-deck/commit/dd4ec04df4fd9b571d1577303d47b4f9ff2d38e6))
* edit a commit message in the commit preview ([24987bc](https://github.com/develar/branch-deck/commit/24987bc3f071c538826f3e7fa4dfc56bdda3d075))
* inline commit message bodies with reading typography ([879f58b](https://github.com/develar/branch-deck/commit/879f58bdd895ea52f0e57be99d2c21ec1fe37f68))
* **model:** switch to Qwen3-4B-Instruct-2507 and remove obsolete model caches ([b6b55a7](https://github.com/develar/branch-deck/commit/b6b55a780c61e31c56de71a3e34715b15f7b4a9a))
* move commits between branches, split an issue branch into pushable branches ([45a889e](https://github.com/develar/branch-deck/commit/45a889edaab4309766096fbab8d4e0a90dd3b8e6))
* one commit row anatomy, quieter branch table and conflict viewer, wider body ([11c6a17](https://github.com/develar/branch-deck/commit/11c6a17e2f46db06e2f9ac8ed871d9f396362601))
* **react:** auto-update, menu sync, auto-sync on focus and theme dialog ([7b28a3d](https://github.com/develar/branch-deck/commit/7b28a3deedd92d918f80246f51abf743a543d16c))
* **react:** branch sync store, branch tables and inline row forms ([9ed746f](https://github.com/develar/branch-deck/commit/9ed746f4bdc0131f6207f7714b878e98e178b2c1))
* **react:** commit list, row selection and inline branch creator ([7ffc7ba](https://github.com/develar/branch-deck/commit/7ffc7ba1597314f10b0c3cdc01d232131f3300c3))
* **react:** conflict viewer and AI branch naming ([1e52bbf](https://github.com/develar/branch-deck/commit/1e52bbf1f1de842cde9da0c1ac389715c11d7342))
* **react:** core infrastructure, settings stores and app shell ([8452005](https://github.com/develar/branch-deck/commit/84520058d4fa1879d203a925b78879d4eb564fc6))
* **react:** repository selection, configuration header and welcome card ([3f9bd6f](https://github.com/develar/branch-deck/commit/3f9bd6fc1ba11308afc68884243761fd53a8b837))
* readable commit message popover with reflow and scrolling ([50177f4](https://github.com/develar/branch-deck/commit/50177f40646aa719017dca293b5e30b852fecf00))


### Bug Fixes

* **amend:** restore the working tree after an aborted autosquash rebase ([18a66b4](https://github.com/develar/branch-deck/commit/18a66b424816054d0585fcb1afc337add3086b97))
* dedupe tokenizers and apply toast theme override ([135807c](https://github.com/develar/branch-deck/commit/135807c172626b0ed46072fae6df82a18305b4b4))
* give the commit title the free width of the branch table ([5c55331](https://github.com/develar/branch-deck/commit/5c55331e426deb82b8cbcd325d1a0a67f42bfd7d)), closes [#72](https://github.com/develar/branch-deck/issues/72)
* history rewrites move the checked out branch only if it did not move ([698ceb5](https://github.com/develar/branch-deck/commit/698ceb599c3e18dd851cca4282392c267dca88a3))
* keep archived branches newest first ([9b9bc33](https://github.com/develar/branch-deck/commit/9b9bc33fd86b48d1afeaf286950e0195da307814))
* keep the push state of a branch across a sync ([7fafdfb](https://github.com/develar/branch-deck/commit/7fafdfbf9a971c1f2d080782d0b8c87204529eeb)), closes [#71](https://github.com/develar/branch-deck/issues/71)
* **model:** keep the partial download when the user pauses ([61a38f5](https://github.com/develar/branch-deck/commit/61a38f5d18895976fce674e24b038957a9dfbb01))
* sync, archive and model bugs found in Rust review ([b17a407](https://github.com/develar/branch-deck/commit/b17a407cb6687d5b42d4fee0fb6016114f3f8211))

## [0.10.0](https://github.com/develar/branch-deck/compare/v0.9.2...v0.10.0) (2025-10-28)


### Features

* add auto-sync on window focus ([fd18d51](https://github.com/develar/branch-deck/commit/fd18d51fa9dfab92ada0c6a4328b32320fe22921)), closes [#45](https://github.com/develar/branch-deck/issues/45)
* amend local changes to a branch ([2ceb6a2](https://github.com/develar/branch-deck/commit/2ceb6a24197f47ab80bde04323ae65c8b57c6ec9))
* support fixup! commits ([9b64d85](https://github.com/develar/branch-deck/commit/9b64d85885ff89491aee8d7361ba7232e1963868)), closes [#51](https://github.com/develar/branch-deck/issues/51)

## [0.9.2](https://github.com/develar/branch-deck/compare/v0.9.1...v0.9.2) (2025-09-11)


### Bug Fixes

* remote status - use path-id based strategy ([8b3292a](https://github.com/develar/branch-deck/commit/8b3292acd0de85d4fac7bd67329ea76d1fc9e04c))

## [0.9.1](https://github.com/develar/branch-deck/compare/v0.9.0...v0.9.1) (2025-09-09)


### Bug Fixes

* inline action progress ([6f89681](https://github.com/develar/branch-deck/commit/6f89681c1c00bc90b4ac014686c65a98bd493c49))

## [0.9.0](https://github.com/develar/branch-deck/compare/v0.8.2...v0.9.0) (2025-09-08)


### Features

* amend local changes to a branch ([fd2ad77](https://github.com/develar/branch-deck/commit/fd2ad7759f41f5dba914e29dc5959964db7fd915))

## [0.8.2](https://github.com/develar/branch-deck/compare/v0.8.1...v0.8.2) (2025-09-04)


### Bug Fixes

* update remote sync on push ([dd7ad83](https://github.com/develar/branch-deck/commit/dd7ad8352b723744ab6da55435bd7337830a10ef))

## [0.8.1](https://github.com/develar/branch-deck/compare/v0.8.0...v0.8.1) (2025-09-01)


### Bug Fixes

* disable add issue reference action if all commits have issue number ([d6c4748](https://github.com/develar/branch-deck/commit/d6c47489c704795f0c595f8f8a3a8f7508416edc))

## [0.8.0](https://github.com/develar/branch-deck/compare/v0.7.1...v0.8.0) (2025-08-25)


### Features

* integration status ([5ce7e7b](https://github.com/develar/branch-deck/commit/5ce7e7b9de4443b5fd2e9dc5aa82d84f40b8d1eb))

## [0.7.1](https://github.com/develar/branch-deck/compare/v0.7.0...v0.7.1) (2025-08-07)


### Bug Fixes

* make AI feature more visible ([ec22728](https://github.com/develar/branch-deck/commit/ec22728dff6bfa28ecf9bc17d58ded8c46278011))

## [0.7.0](https://github.com/develar/branch-deck/compare/v0.6.0...v0.7.0) (2025-08-02)


### Features

* unassigned commits - AI branch name generation ([a6324ff](https://github.com/develar/branch-deck/commit/a6324ff9db665e285781b0a83b196de618ab5c06))

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
