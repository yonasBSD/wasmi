# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Additionally we have an `Internal` section for changes that are of interest to developers.

Dates in this file are formattes as `YYYY-MM-DD`.

## `0.48.0` - 2025-07-21

### Added

- Added Wasm `reinterpret` operators to `wasmi_core::wasm` API. 

### Changed

- Marked `Module::new_streaming[_unchecked]` API deprecated. [#1540]
  - Reason for the deprecation:
    - The streaming Wasm module creation is not a great fit for Wasmi's target usage.
    - No users are known that depend on this functionality - please inform us if you do!
    - Streaming Wasm module creating has a performance overhead when not needed.
- Changed `CompilationMode` default to `CompilationMode::LazyTranslation`. [#1530]
  - With this default Wasm is still validated eagerly but tranlated to Wasmi IR lazily.
    This gives the best of both worlds: fast startup times while avoiding partial validation.
- Update Wasmtime dependencies to v34. [#1563]

### Fixed

- Fixed a bug that `f{32,64}.copysign` with immediate `rhs` operands won't bump fuel. [#1539]
- Fixed incorrect optimization application for `i64.mul_wide_s`. [#1545] [#1546]
- Fixed an `integer-overflow` that could happen when reading or writing memory. [#1554]

### Internal

- Optimize `ControlFrame` memory footprint slightly. [#1534]
- Slightly optimize `fuse_nez` and `fuse_eqz` routines. [#1559]
- Simplified lots of `FuncTranslator` logic. E.g. [#1542] [#1550] [#1552]
- Add some new Wasmi specific `.wast` test cases. [#1561] [#1562]
- Prepare for new Wasmi translator implementation.
  - [#1532] [#1537] [#1541] [#1553] [#1558] [#1560]

[#1530]: https://github.com/wasmi-labs/wasmi/pull/1530
[#1534]: https://github.com/wasmi-labs/wasmi/pull/1534
[#1539]: https://github.com/wasmi-labs/wasmi/pull/1539
[#1540]: https://github.com/wasmi-labs/wasmi/pull/1540
[#1542]: https://github.com/wasmi-labs/wasmi/pull/1542
[#1545]: https://github.com/wasmi-labs/wasmi/pull/1545
[#1546]: https://github.com/wasmi-labs/wasmi/pull/1546
[#1550]: https://github.com/wasmi-labs/wasmi/pull/1550
[#1552]: https://github.com/wasmi-labs/wasmi/pull/1552
[#1554]: https://github.com/wasmi-labs/wasmi/pull/1554
[#1555]: https://github.com/wasmi-labs/wasmi/pull/1555
[#1559]: https://github.com/wasmi-labs/wasmi/pull/1559
[#1561]: https://github.com/wasmi-labs/wasmi/pull/1561
[#1562]: https://github.com/wasmi-labs/wasmi/pull/1562
[#1563]: https://github.com/wasmi-labs/wasmi/pull/1563
[#1532]: https://github.com/wasmi-labs/wasmi/pull/1532
[#1537]: https://github.com/wasmi-labs/wasmi/pull/1537
[#1541]: https://github.com/wasmi-labs/wasmi/pull/1541
[#1553]: https://github.com/wasmi-labs/wasmi/pull/1553
[#1558]: https://github.com/wasmi-labs/wasmi/pull/1558
[#1560]: https://github.com/wasmi-labs/wasmi/pull/1560

## `0.47.0` - 2025-05-30

### Changed

- Remove the `downcast-rs` dependency from Wasmi crates. [#1517]
- Bump the minimum supported Rust version (MSRV) to Rust 1.86. [#1518]
  - This change was necessitated by the removal of `downcast-rs`.

### Internal

- Lower `select` instructions more aggressively. [#1526]
  - This significantly reduced the number of different `select` instruction
    variants and thus might have positive affects on Wasmi's execution performance.

[#1517]: https://github.com/wasmi-labs/wasmi/pull/1517
[#1518]: https://github.com/wasmi-labs/wasmi/pull/1518
[#1526]: https://github.com/wasmi-labs/wasmi/pull/1526

## `0.46.0` - 2025-05-08

### Changed

- `Store<T>::new` no longer requires `T: 'static`. [#1507]
    - The `T: 'static` requirement was introduced in `v0.45.0` in [#1449].
- Improve host function call performance. [#1506]

### Internal

- Updated dependencies. [#1509]

[#1506]: https://github.com/wasmi-labs/wasmi/pull/1506
[#1507]: https://github.com/wasmi-labs/wasmi/pull/1507
[#1509]: https://github.com/wasmi-labs/wasmi/pull/1509

## `0.45.0` - 2025-05-06

### Added

- Added support for Wasm function call resumption after running out of fuel. [#1498]
  - This feature is very useful when using Wasmi inside a scheduler that works with
    Wasmi's fuel metering to provide amount of compute units to different Wasm execution
    threads for example.
- Added missing `wasmi_core::simd` API functions for `relaxed-simd`. [#1447]
- Added implementations for Rust's `Error` trait for all  `wasmi` error types on `no_std`. [#1462]

### Changed

- Avoid performing duplicate type and validation checks in `Linker::instantiate`. [#1476]
- Updated `wasm-tools` dependencies to v228. [#1463]
- Removed most of `wasmi_core::TypedVal`'s API. [#1457]
  - The newer `wasmi_core::wasm` API is to be preferred and provides the same functionality.

### Fixed

- Fixed a bug that Wasmi did not make `wasmparser`'s parser aware of enabled Wasm features. [#1502]
  - Making `wasmparser` aware of the enabled Wasm features allows it to detect malformed Wasm
    binaries during parsing.

### Internal

- Make Wasmi's executor non-generic over the `Store`'s `T`. [#1449]
- Changes to Wasmi's IR:
  - Removed all conditional return instructions. [#1486]
    - This allows Wasmi to apply its powerful cmp+branch fusion in more places.
  - Remove most of the `bulk-memory` (and `bulk-table`) instruction variants. [#1489]
    - Wasmi still has optimized variants for the most common cases.
  - Add new logical-comparator instructions. [#1494]
    - This further enhances Wasmi's powerful cmp+branch instruction fusion.
  - Add negated `f{32,64}.{lt,le}` instructions. [#1496]
    - This allows Wasmi to apply its cmp+nez fusion for `f{32,64}.{le,lt}` instructions as well.
  - Re-design Wasmi's `select` instructions. [#1497]
    - This allows to use Wasmi's powerful cmp op-code fusion for `select` instructions.
- Moved many `wasmi` internals into `wasmi_core`:
  - Add `FuncType` [#1458]
  - Add `Fuel`, `Memory`, `Table`, `Global`, `ResourceLimiter` [#1464]
  - Replace uses in `wasmi` with `wasmi_core` definitions. [#1460]

[#1447]: https://github.com/wasmi-labs/wasmi/pull/1447
[#1449]: https://github.com/wasmi-labs/wasmi/pull/1449
[#1457]: https://github.com/wasmi-labs/wasmi/pull/1457
[#1458]: https://github.com/wasmi-labs/wasmi/pull/1458
[#1460]: https://github.com/wasmi-labs/wasmi/pull/1460
[#1462]: https://github.com/wasmi-labs/wasmi/pull/1462
[#1463]: https://github.com/wasmi-labs/wasmi/pull/1463
[#1464]: https://github.com/wasmi-labs/wasmi/pull/1464
[#1476]: https://github.com/wasmi-labs/wasmi/pull/1476
[#1486]: https://github.com/wasmi-labs/wasmi/pull/1486
[#1489]: https://github.com/wasmi-labs/wasmi/pull/1489
[#1494]: https://github.com/wasmi-labs/wasmi/pull/1494
[#1496]: https://github.com/wasmi-labs/wasmi/pull/1496
[#1497]: https://github.com/wasmi-labs/wasmi/pull/1497
[#1498]: https://github.com/wasmi-labs/wasmi/pull/1498
[#1502]: https://github.com/wasmi-labs/wasmi/pull/1502

## `0.44.1` - 2025-05-04

### Fixed

- Fixed a bug with executing SIMD `store_lane` instructions. [#1450]

[#1450]: https://github.com/wasmi-labs/wasmi/pull/1450

## `0.44.0` - 2025-03-29

### Added

- Add support for the Wasm `relaxed-simd` proposal. [#1443]
  - All `relaxed-simd` operators behave deterministically on all platforms supported by Wasmi.
  - Users have to enable the `simd` crate feature in order to use `relaxed-simd` capabilities.
  - Note that enabling the `simd` crate feature may regress Wasm execution and memory consumption
    performance.

### Changed

- Wasmi's CLI now prints multiple results on a new line each. [#1438]
  - With this change Wasmi's CLI and Wasmtime's CLI have the same behavior.

[#1438]: https://github.com/wasmi-labs/wasmi/pull/1438
[#1443]: https://github.com/wasmi-labs/wasmi/pull/1443

## `0.43.1` - 2025-03-29

### Fixed

- Add missing `WasmTy` implementation for `V128` [#1437]
  - This prevented using `V128` parameters and results in the `TypedFunc` API.
  - Note that it was still possible to use `V128` with the `Func::call` API.
- Fixed a bug executing `i8x16.replace_lane` with immediate parameter. [#1444]

[#1437]: https://github.com/wasmi-labs/wasmi/pull/1437
[#1444]: https://github.com/wasmi-labs/wasmi/pull/1444

## `0.43.0` - 2025-03-27

### Added

- Added support for the Wasm `simd` proposal. [#1364]
  - Users have to opt-in to use this feature by enabling Wasmi's `simd` crate feature.
  - Note: enabling `simd` may introduce Wasm execution overhead, increase memory consumption
          increase compiled artifact size and compile times for Wasmi crates. So use `simd`
          only if your use case needs it.

### Changed

- Wasmi's minimum supported Rust version is now Rust 1.83. [#1405]

### Internal

- Move all Wasm spec tests over to the `wasmi_wast` crate. [#1403]
  - This solves a cyclic dev-dependency issue between `wasmi` and `wasmi_wast` crates.
- Updated `wasm-tools` and Wasmtime dependencies. [#1404]

[#1364]: https://github.com/wasmi-labs/wasmi/issues/1364
[#1403]: https://github.com/wasmi-labs/wasmi/pull/1403
[#1404]: https://github.com/wasmi-labs/wasmi/pull/1404
[#1405]: https://github.com/wasmi-labs/wasmi/pull/1405

## `0.42.1` - 2025-03-20

### Fixed

- Fixed a bug in `i64.mul_wide_{s,u}` instruction constant evaluation. [#1397]

[#1397]: https://github.com/wasmi-labs/wasmi/pull/1397

## `0.42.0` - 2025-03-11

### Added

- Added support for the Wasm `wide-arithmetic` proposal. [#1369]
    - The `wide-arithmetic` proposal is disabled by default in `wasmi`
      library and enabled by default in the Wasmi CLI.

### Changed

- Optimized memory accesses with a constant `ptr` value. [#1381]

### Internal

- Update `wasm-tools` dependencies from v226 to v227. [#1380]

[#1369]: https://github.com/wasmi-labs/wasmi/pull/1369
[#1380]: https://github.com/wasmi-labs/wasmi/pull/1380
[#1381]: https://github.com/wasmi-labs/wasmi/pull/1381

## `0.41.1` - 2025-03-11

### Fixed

- Fixed a Wasmi CLI crash when using `.wat` formatted Wasm files. [#1385]
- Fixed a crash when translating `memory.grow` with an `i64.const` delta parameter. [#1384]
  - Note: this can only occur when using the Wasm `memory64` proposal.

[#1384]: https://github.com/wasmi-labs/wasmi/pull/1384
[#1385]: https://github.com/wasmi-labs/wasmi/pull/1385

## `0.41.0` - 2025-03-10

### Added

- Added support for the Wasm `memory64` proposal. [#1371]
    - The `memory64` proposal is enabled by default in `wasmi` and the Wasmi CLI.
- Added support for the Wasm `custom-page-sizes` proposal. [#1349]
    - The `custom-page-sizes` proposal is enabled by default in `wasmi` and the Wasmi CLI.
- Added support to for Wat inputs in `Module::new` and `Module::new_unchecked`. [#1328]
    - There deliberately is no Wat support in `Module::new_streaming` since Wat cannot be stream compiled.

### Fixed

- Fixed a bug that could lead to crashes when tail calling host functions. [#1329]
- Fixed a bug that `no_mange` and `export_name` where used at the same time. [#1337]

### Changed

- Bumped Minimum Support Rust Version to v1.82. [#1375]
- The `memory.grow` and `table.grow` instructions now trap instead of panic when out of system memory.
  - This change was part of the changes introduced by the support
    for the Wasm `memory64` and `custom-page-sizes` proposals.

### Internal

- No longer use `libm` default features. [#1322]
- Implemented several improvements to our fuzzing infrastructure:
    - Significantly improved Wasmtime translation (JIT) times. [#1339]
    - Improve debug output of fuzzers. [#1344]
    - Differential fuzzer now uses fuzz input to randomize function parameters. [#1348]
    - Allow fuzzing the Wasm `custom-page-sizes` proposal implementation. [#1354]
    - Allow fuzzing the Wasm `memory64` proposal implementation. [#1379]
- Update the Wasm spec testsuite. [#1361]
- Update `wasm-tools` dependencies to v226. [#1374]
- Update to `string-interner` v0.19. [#1367]

[#1322]: https://github.com/wasmi-labs/wasmi/pull/1322
[#1328]: https://github.com/wasmi-labs/wasmi/pull/1328
[#1329]: https://github.com/wasmi-labs/wasmi/pull/1329
[#1337]: https://github.com/wasmi-labs/wasmi/pull/1337
[#1339]: https://github.com/wasmi-labs/wasmi/pull/1339
[#1344]: https://github.com/wasmi-labs/wasmi/pull/1344
[#1348]: https://github.com/wasmi-labs/wasmi/pull/1348
[#1349]: https://github.com/wasmi-labs/wasmi/pull/1349
[#1354]: https://github.com/wasmi-labs/wasmi/pull/1354
[#1361]: https://github.com/wasmi-labs/wasmi/pull/1361
[#1367]: https://github.com/wasmi-labs/wasmi/pull/1367
[#1375]: https://github.com/wasmi-labs/wasmi/pull/1375
[#1374]: https://github.com/wasmi-labs/wasmi/pull/1374
[#1371]: https://github.com/wasmi-labs/wasmi/pull/1371
[#1379]: https://github.com/wasmi-labs/wasmi/pull/1379

## `0.40.0` - 2024-11-27

This release focuses on compile time improvements for Wasmi,
significantly reducing the time it takes to compile Wasmi and
decrease its compiled artifact size.

### Added

- Added optimization for `load` and `store` lowering. [#1303]
    - This reduces the total number of Wasmi instructions.
- Added `prefix-symbols` crate feature to `wasmi_c_api_impl` crate. [#1315]
    - This allows to prefix all exported symbols with `wasmi_` in order to
      avoid duplicate symbols when linking multiple Wasm runtimes implementing
      the Wasm C-API.

### Fixed

- C-API
    - Fix a minor compilation issue. [#1296]
    - Fix the name of `wasmi_config_compilation_mode_set` [#1298]
- Conditionally forward the `string-interner/std` crate feature. [#1304]
- Fix Wasmtime fuzzer oracle config usage. [#1314]

### Changed

- Bumped minimum supported Rust version from v1.79 -> v1.80. [#1318]
- Replace the `wasmparser-nostd` fork with upstream `wasmparser`. [#1141]
    - This allows Wasmi to implement new Wasm proposals.
    - Unfortunately this update also regresses Wasmi translation performance
      by roughly 5-15% depending on the exact Wasm blob and translation mode.
- Update the `string-interner` and `hashbrown` dependencies. [#1305]

### Internal

- Update the `wast` dependency for Wasmi's Wast runner. [#1306]
- Update `wasm-tools` dependencies to `v0.221`. [#1318]

[#1141]: https://github.com/wasmi-labs/wasmi/pull/1141
[#1296]: https://github.com/wasmi-labs/wasmi/pull/1296
[#1298]: https://github.com/wasmi-labs/wasmi/pull/1298
[#1303]: https://github.com/wasmi-labs/wasmi/pull/1303
[#1304]: https://github.com/wasmi-labs/wasmi/pull/1304
[#1305]: https://github.com/wasmi-labs/wasmi/pull/1305
[#1306]: https://github.com/wasmi-labs/wasmi/pull/1306
[#1314]: https://github.com/wasmi-labs/wasmi/pull/1314
[#1315]: https://github.com/wasmi-labs/wasmi/pull/1315
[#1318]: https://github.com/wasmi-labs/wasmi/pull/1318

## `0.39.1` - 2024-11-06

### Fixed

- Fixed a bug when translating double negations in expression contexts. [#1293]

[#1293]: https://github.com/wasmi-labs/wasmi/pull/1293

## `0.39.0` - 2024-11-04

### Added

- Add new `Linker` APIs. [#1281]
  - `Linker::instance`: conveniently add exports from an instance to a linker.
  - `Linker::alias_module`: alias module definitions via another name.
  - `Linker::allow_shadowing`: enable to shadow previous definitions without errors.
- Add `hash-collections` and `prefer-btree-collections` crate features to the `wasmi` crate. [#1265]
    - This allows for more fine grained control over Wasmi
      dependencies to further decrease compile times.
- Add lowering of compare instructions and fused branch+compare instructions. [#1243]
    - This improved performance for certain workloads and
      reduced the total Wasmi instruction count significantly.

### Fixed

- Fixed a bug in translation of fused `cmp+branch` instructions with huge offsets.
  - This was fixed as a side product in [#1243].

### Removed

- Removed the `no-hash-maps` crate feature. [#1265]
- Removed some minor `wasmi` crate dependencies. [#1266] [#1267]
  - This should improve compile times of the `wasmi` crate slightly.

### Internal

- Modernize fuzzer and significantly improve fuzzing test coverage.
  - Reworked `differential` fuzzing entirely. [#1257]
    - This also improves handling of non-deterministic behavior
      between Wasm runtimes in `differential` fuzzing.
  - Add `wasmi_fuzz` crate for better code organization. [#1252]
  - Merged `translate` and `translate_metered` fuzzers. [#1249]
- Modernize Wasmi `.wast` directives runner. [#1279]
  - Overall this significantly improved readability and maintainability
    of the Wasmi `.wast` directives runner.

[#1243]: https://github.com/wasmi-labs/wasmi/pull/1243
[#1249]: https://github.com/wasmi-labs/wasmi/pull/1249
[#1252]: https://github.com/wasmi-labs/wasmi/pull/1252
[#1257]: https://github.com/wasmi-labs/wasmi/pull/1257
[#1265]: https://github.com/wasmi-labs/wasmi/pull/1265
[#1266]: https://github.com/wasmi-labs/wasmi/pull/1266
[#1267]: https://github.com/wasmi-labs/wasmi/pull/1267
[#1279]: https://github.com/wasmi-labs/wasmi/pull/1279
[#1281]: https://github.com/wasmi-labs/wasmi/pull/1281

## `0.38.0` - 2024-10-06

### Added

- Add `no-hash-maps` crate feature to Wasmi CLI and enable it by default. [#1225]

### Internal

- Rename various instructions and add `ShiftAmount` abstraction. [#1221]
- Use Rust's `ControlFlow` utility. [#1223]
- Use `get_memory` in `load` and `store` execution handlers. [#1224]

[#1221]: https://github.com/wasmi-labs/wasmi/pull/1221
[#1223]: https://github.com/wasmi-labs/wasmi/pull/1223
[#1224]: https://github.com/wasmi-labs/wasmi/pull/1224
[#1225]: https://github.com/wasmi-labs/wasmi/pull/1225

## `0.37.2` - 2024-10-04

### Added

- Added a new `extra-checks` crate feature to the `wasmi` crate. [#1217]

    - This improves unreachability checks in when `debug-assertions` or `extra-checks` are enabled.
    - If `extra-checks` are disabled, some technically unnecessary runtime checks are no longer performed.
    - Use `extra-checks` if your focus is on safety, disable if your focus is on performance.

### Fixed

- Fixed a bug in local preservation when translating Wasm `loop` control flow. [#1218]

[#1217]: https://github.com/wasmi-labs/wasmi/pull/1217
[#1218]: https://github.com/wasmi-labs/wasmi/pull/1218

## `0.37.1` - 2024-10-01

### Fixed

- Fixed a bug in `select` translation constant propagation. [#1213]

[#1213]: https://github.com/wasmi-labs/wasmi/pull/1213

## `0.37.0` - 2024-09-30

### Added

- Added support for Wasm `multi-memory` proposal. [#1191]
- Added `Store::call_hook` API. [#1144]
  - Contributed by [emiltayl](https://github.com/emiltayl).

### Changed

- Updated WASI dependencies. [#1140]
  - This fixes some long-standing bugs in the `wasmi_wasi` crate.

### Fixed

- This release includes all fixes that have been backported to `v0.36.1`, `v0.36.2` and `v0.36.5`.

### Internal

- Add new Wasmi bytecode. [#1152]
  - This was a major undertaking with lots of sub-issues and PRs.
  - The Wasmi bytecode definitions now reside in their own [`wasmi_ir` crate].
  - Most of the definitions are sourced from a single Rust macro to reduce maintenance friction.
- Remove unnecessary `iextend` instructions. [#1147]
- Changed encoding for Wasmi `call_indirect` instructions. [#1156]
  - The new encoding improves performance and reduces the number of function local constants.
- Changed encoding for Wasmi `select` instructions. [#1157]
  - The new encoding is more straight-forward and aims to simplify the Wasmi executor and translator.
- Changed encoding for Wasmi `br_table` instruction. [#1158]
  - The new encoding improves performance and memory consumption for certain use cases.
- Minor improvements to Wasmi bytecode.
  - `MemoryGrowBy` now takes `u32` delta. [#1193]
  - Improved `storeN` encoding with immediates. [#1194]

[#1144]: https://github.com/wasmi-labs/wasmi/pull/1144
[#1147]: https://github.com/wasmi-labs/wasmi/pull/1147
[#1140]: https://github.com/wasmi-labs/wasmi/pull/1140
[#1152]: https://github.com/wasmi-labs/wasmi/pull/1152
[#1156]: https://github.com/wasmi-labs/wasmi/pull/1156
[#1157]: https://github.com/wasmi-labs/wasmi/pull/1157
[#1158]: https://github.com/wasmi-labs/wasmi/pull/1158
[#1191]: https://github.com/wasmi-labs/wasmi/pull/1191
[#1193]: https://github.com/wasmi-labs/wasmi/pull/1193
[#1194]: https://github.com/wasmi-labs/wasmi/pull/1194

[`wasmi_ir` crate]: https://crates.io/crates/wasmi_ir

## `0.36.5` - 2024-10-11

### Fixed

- Fixed a bug with `table.get` translation when `index` is a preserved register. [#commit-b4e78d]

[#commit-82c938]: https://github.com/wasmi-labs/wasmi/commit/82c9388f1d54e4e74e1b581f11978b4028eeaba2

## `0.36.4` - 2024-10-03

### Fixed

- Fixed a bug in local preservation when translating Wasm `loop` control flow. [#1218]

[#1218]: https://github.com/wasmi-labs/wasmi/pull/1218

## `0.36.3` - 2024-10-01

### Fixed

- Fixed a bug in `select` translation constant propagation. [#1213]

[#1213]: https://github.com/wasmi-labs/wasmi/pull/1213

## `0.36.2` - 2024-09-28

### Fixed

- Fix miri reported UB in `FuncRef` and `ExternRef` conversions. [#1201]
- Fix bug in `table.init` from imported `global.get` values. [#1192]

### Changed

- Changed some `inline` annotations in the Wasmi executor. [#commit-b4e78d]
    - This change had minor positive effects on the performance of commonly executed Wasmi instructions.

[#1192]: https://github.com/wasmi-labs/wasmi/pull/1192
[#1201]: https://github.com/wasmi-labs/wasmi/pull/1201
[#commit-b4e78d]: https://github.com/wasmi-labs/wasmi/commit/b4e78d23451cb40a7b43404f8e6e868a362b7985

## `0.36.1` - 2024-09-20

### Fixed

- Fixed `ref.is_null` translation constant propagation issue. [#1189]
- Fixed invalid overwrite of preserved local register. [#1177]
- Removed faulty `br_table` optimization.
    - [Link to Commit](https://github.com/wasmi-labs/wasmi/commit/a646d27a4d69e73dffb30bf706bfb394dfa6a27f)
- Fix a few `clippy` warnings.

[#1177]: https://github.com/wasmi-labs/wasmi/pull/1177
[#1189]: https://github.com/wasmi-labs/wasmi/pull/1189

## `0.36.0` - 2024-07-24

### Added

- Added support for the official Wasm C-API. (https://github.com/wasmi-labs/wasmi/pull/1009)
  - This allows to use Wasmi from any program that can interface with C code.
  - The `wasmi_c_api_impl` crate allows to use Wasmi via the Wasm C-API from Rust code.
- Added `Instance::new` API. (https://github.com/wasmi-labs/wasmi/pull/1134)
  - This was mainly needed to support the Wasm C-API.
  - The new API offers a more low-level way for Wasm module instantiation
    that may be more efficient for certain use cases.
- Added `Clone` implementation for `Module`. (https://github.com/wasmi-labs/wasmi/pull/1130)
  - This was mainly needed to support the Wasm C-API.

### Changed

- The store fuel API now returns `Error` instead of `FuelError`. (https://github.com/wasmi-labs/wasmi/pull/1131)
  - This was needed to support the Wasm C-API.
  - The `FuelError` is still accessible via the `Error::kind` method.

## `0.35.0` - 2024-07-11

### Fixed

- Fixed a dead-lock that prevented users from compiling Wasm modules in host functions
  called from Wasmi's executor. (https://github.com/wasmi-labs/wasmi/pull/1122)
    - This was a very long-standing bug in the Wasmi interpreter and it is now finally closed.
    - Note that this regressed performance of call-intense workloads by roughly 5-10%.
      Future work is under way to hopefully fix these regressions.
    - Before this fix, users had to use a work-around using resumable function calls to
      cirumvent this issue which is no longer necessary, fortunately.

### Internals

- Add `CodeMap::alloc_funcs` API and use it when compiling Wasm modules. (https://github.com/wasmi-labs/wasmi/pull/1125)
    - This significantly improved performance for lazily compiling
      Wasm modules (e.g.  via `Module::new`) by up to 23%.

## `0.34.0` - 2024-07-08

### Added

- Allows Wasmi CLI to be installed with locked dependencies. ([#1096])
    - This can be done as follows: `cargo install --locked wasmi_cli`

### Fixed

- Allow Wasm module instantiation in host functions called from Wasmi's executor. ([#1116])

### Changed

- Limit number of parameter and result types in `FuncType` to 1000, each. ([#1116])

### Dev. Note

- Significantly improved and Wasmi's CI and made it a lot faster.
    - Multi PR effort: [#1098], [#1100], [#1104]
- Refactored and cleaned-up call based and Rust sourced Wasmi benchmarks.
    - Call-based Benchmarks: [#1102], [#1113]
    - Rust-sourced Benchmarks: [#1107], [#1108], [#1109], [#1111], [#1115]

[#1096]: https://github.com/wasmi-labs/wasmi/pull/1096
[#1098]: https://github.com/wasmi-labs/wasmi/pull/1098
[#1100]: https://github.com/wasmi-labs/wasmi/pull/1100
[#1102]: https://github.com/wasmi-labs/wasmi/pull/1102
[#1104]: https://github.com/wasmi-labs/wasmi/pull/1104
[#1107]: https://github.com/wasmi-labs/wasmi/pull/1107
[#1108]: https://github.com/wasmi-labs/wasmi/pull/1108
[#1109]: https://github.com/wasmi-labs/wasmi/pull/1109
[#1111]: https://github.com/wasmi-labs/wasmi/pull/1111
[#1113]: https://github.com/wasmi-labs/wasmi/pull/1113
[#1115]: https://github.com/wasmi-labs/wasmi/pull/1115
[#1116]: https://github.com/wasmi-labs/wasmi/pull/1116

## `0.33.1` - 2024-07-01

### Added

- Added `Error` trait impls for all Wasmi error types impleemnting `Display`. (https://github.com/wasmi-labs/wasmi/pull/1089)
    - Contributed by [kajacx](https://github.com/kajacx).

### Fixed

- Fixed compilation for Rust versions <1.78. (https://github.com/wasmi-labs/wasmi/pull/1093)
- Fixed nightly `clippy` warning about `map_err`. (https://github.com/wasmi-labs/wasmi/pull/1094)

## `0.33.0` - 2024-06-24

### Added

- Added support for Wasm custom sections processing. (https://github.com/wasmi-labs/wasmi/pull/1085)
    - It is now possible to query name and data of Wasm custom sections of a `Module`.
    - Use the new `Config::ignore_custom_sections` flag to disable this functionality.
- Added `Config::ignore_custom_sections` flag to disable processing custom sections if this is unwanted. (https://github.com/wasmi-labs/wasmi/pull/1085)
- Add `Memory::{data_ptr, data_size, size}` methods. (https://github.com/wasmi-labs/wasmi/pull/1082)
- Added a Wasmi usage guide documentation. (https://github.com/wasmi-labs/wasmi/pull/1072)
    - Link: https://github.com/wasmi-labs/wasmi/blob/master/docs/usage.md

### Changed

- Optimized the Wasmi executor in various ways.
    - In summary the Wasmi executor now more optimally caches the currently used
      Wasm instance and optimizes access to instance related data.
      In particular access to the default linear memory bytes as well as the value of
      the global variable at index 0 (often used as shadow stack pointer) are more efficient.
    - The following PRs are part of this effort:
        - https://github.com/wasmi-labs/wasmi/pull/1059
        - https://github.com/wasmi-labs/wasmi/pull/1062
        - https://github.com/wasmi-labs/wasmi/pull/1068
        - https://github.com/wasmi-labs/wasmi/pull/1069
        - https://github.com/wasmi-labs/wasmi/pull/1065
        - https://github.com/wasmi-labs/wasmi/pull/1075
        - https://github.com/wasmi-labs/wasmi/pull/1076
- Changed `Memory::grow` signature to mirror Wasmtime's `Memory::grow` method. (https://github.com/wasmi-labs/wasmi/pull/1082)

### Removed

- Removed `Memory::current_pages` method. (https://github.com/wasmi-labs/wasmi/pull/1082)
    - Users should use the new `Memory::size` method instead.

## `0.32.3` - 2024-06-06

### Fixed

- Fix overlapping reuse of local preservation slots. (https://github.com/wasmi-labs/wasmi/pull/1057)
    - Thanks again to [kaiavintr](https://github.com/kaiavintr) for reporting the bug.

## `0.32.2` - 2024-06-03

### Fixed

- Refine and generalize the fix for v0.32.1. (https://github.com/wasmi-labs/wasmi/pull/1054)

## `0.32.1` - 2024-06-03

### Fixed

- Fixes a miscompilation when merging two copy instructions where the result of the first copy is also the input to the second copy and vice versa. (https://github.com/wasmi-labs/wasmi/pull/1052)
    - Thanks to [kaiavintr](https://github.com/kaiavintr) for reporting the bug.

## `0.32.0` - 2024-05-28

**Note:**

- This release is the culmination of months of research, development and QA
  with a new execution engine utilizing register-based IR at its core boosting
  both startup and execution performance to new levels for the Wasmi interpreter.
- This release is accompanied with [an article](https://wasmi-labs.github.io/blog/) that presents some of the highlights.

### Added

- Added a new execution engine based on register-based bytecode. (https://github.com/wasmi-labs/wasmi/pull/729)
    - The register-based Wasmi `Engine` executes roughly 80-100% faster and
      compiles roughly 30% slower according to benchmarks conducted so far.
- Added `Module::new_unchecked` API. (https://github.com/wasmi-labs/wasmi/pull/829)
    - This allows to compile a Wasm module without Wasm validation which can be useful
      when users know that their inputs are valid Wasm binaries.
    - This improves Wasm compilation performance for faster startup times by roughly 10-20%.
- Added Wasm compilation modes. (https://github.com/wasmi-labs/wasmi/pull/844)
    - When using `Module::new` Wasmi eagerly compiles Wasm bytecode into Wasmi bytecode
      which is optimized for efficient execution. However, this compilation can become very
      costly especially for large Wasm binaries.
    - The solution to this problem is to introduce new compilation modes, namely:
      - `CompilationMode::Eager`: Eager compilation, what Wasmi did so far. (default)
      - `CompilationMode::LazyTranslation`: Eager Wasm validation and lazy Wasm translation.
      - `CompilationMode::Lazy`: Lazy Wasm validation and translation.
    - Benchmarks concluded that
      - `CompilationMode::LazyTanslation`: Usually improves startup performance by a factor of 2 to 3.
      - `CompilationMode::Lazy`: Usually improves startup performance by a factor of up to 27.
    - Note that `CompilationMode::Lazy` can lead to partially validated Wasm modules
      which can introduce non-determinism when using different Wasm implementations.
      Therefore users should know what they are doing when using `CompilationMode::Lazy` if this is a concern.
    - Enable lazy Wasm compilation with:
      ```rust
      let mut config = wasmi::Config::default();
      config.compilation_mode(wasmi::CompilationMode::Lazy);
      ```
    - When `CompilationMode::Lazy` or `CompilationMode::LazyTranslation` and fuel metering is enabled
      the first function access that triggers compilation (and validation) will charge fuel respective
      to the number of bytes of the Wasm function body. (https://github.com/wasmi-labs/wasmi/pull/876)
- Added non-streaming Wasm module compilation. (https://github.com/wasmi-labs/wasmi/pull/1035)
    - So far Wasmi only offered a streaming Wasm module compilation API, however most users
      probably never really needed that. So starting from this version both `Module::new` and
      `Module::new_unchecked` are now non-streaming with insane performance improvements of up
      to 80% in certain configurations.
    - For streaming Wasm module users we added `Module::new_streaming` and `Module::new_streaming_unchecked` APIs.
- Added `Module::validate` API. (https://github.com/wasmi-labs/wasmi/pull/840)
    - This allows to quickly check if a Wasm binary is valid according to a Wasmi `Engine` config.
    - Note that this does not translate the Wasm and thus `Module::new` or `Module::new_unchecked`
      might still fail due to translation errors.
- CLI: Added `--compilation-mode` argument to enable lazy Wasm compilation. (https://github.com/wasmi-labs/wasmi/pull/849)
- Added `--verbose` mode to Wasmi CLI by @tjpalmer. (https://github.com/wasmi-labs/wasmi/pull/957)
    - By default Wasmi CLI no longer prints messages during execution.
- Added `Memory::new_static` constructor by @Ddystopia. (https://github.com/wasmi-labs/wasmi/pull/939)
    - This allows to construct a Wasm `Memory` from a static byte array
      which is especially handy for certain embedded use cases.
- Added `LinkerBuilder` type. (https://github.com/wasmi-labs/wasmi/pull/989)
    - Using `LinkerBuilder` to create new `Linker`s with the same set of host functions is a lot more
      efficient than creating those `Linker`s the original way. However, the initial `LinkerBuilder`
      construction will be as inefficient as building up a `Linker` previously.
- Added `EnforcedLimits` configuration option to `Config`. (https://github.com/wasmi-labs/wasmi/pull/985)
    - Some users want to run Wasm binaries in a specially restricted or limited mode.
      For example this mode limits the amount of functions, globals, tables etc. can be defined
      in a single Wasm module.
      With this change they can enable this new strict mode using
      ```rust
      let mut config = wasmi::Config::default();
      config.enforced_limits(wasmi::EnforcedLimits::strict());
      ```
      In future updates we might relax this to make `EnforcedLimits` fully customizable.
- Added `EngineWeak` constructed via `Engine::weak`. (https://github.com/wasmi-labs/wasmi/pull/1003)
    - This properly mirrors the Wasmtime API and allows users to store weak references to the `Engine`.
- Added `no-hash-maps` crate feature to the `wasmi` crate. (https://github.com/wasmi-labs/wasmi/pull/1007)
    - This tells the `wasmi` crate to avoid using hash based data structures which can be beneficial for
      running Wasmi in some embedded environments such as `wasm32-unknown-unknown` that do not support
      random sources and thus are incapable to spawn hash maps that are resilient to malicious actors.
    - Note that Wasmi has always avoided using hash map based data structures prior to this change so
      not enabling this new crate feature kind of acts as an optimization.
- Added `Default` implementation for `Store<T> where T: Default`. (https://github.com/wasmi-labs/wasmi/pull/1031)
    - This mostly serves as a convenient way to create a minimal Wasmi setup.
- Added `WasmTy` implementations for `f32` and `f64` Rust primitives. (https://github.com/wasmi-labs/wasmi/pull/1031)
    - This is convenience for `Linker::func_wrap` calls that take those primitives as arguments.
      Before this change users had to use `F32` and `F64` instead which is a bit cumbersome.

### Changed

- Minimum Rust version set to 1.77. (https://github.com/wasmi-labs/wasmi/pull/961)
- CLI: Enabled Wasm `tail-calls` and `extend-const` proposals by default. (https://github.com/wasmi-labs/wasmi/pull/849)
    - We expect those Wasm proposals to be stabilized very soon so we feel safe to enable them by default already.
- Improved `Debug` and `Display` impls for NaNs of Wasm `f32` and `f64` values.
  - They now show `nan:0x{bytes}` where `{bytes}` is their respective raw bytes.
- Implement `Sync` for `ResumableInvocation` and `TypedResumableInvocation`. (https://github.com/wasmi-labs/wasmi/pull/870)
- Properly mirror Wasmtime's fuel API. (https://github.com/wasmi-labs/wasmi/pull/1002)
- Renamed some Wasmi items to improve its Wasmtime mirroring. (https://github.com/wasmi-labs/wasmi/pull/1011)
- Improved Wasmtime API mirror for Store fuel. (https://github.com/wasmi-labs/wasmi/pull/1002)
- Enabled `Config::tail_call` and `Config::extended_const` by default. (https://github.com/wasmi-labs/wasmi/pull/1031)
    - Those Wasm proposals have been moved to phase 4 for many months now.

### Removed

- Removed the stack-machine bytecode based Wasmi `Engine` backend. (https://github.com/wasmi-labs/wasmi/pull/818)
    - The new register-based bytecode based Wasmi `Engine` is more promising
      and the Wasmi team does not want to maintain two different engine backends.
- Removed `FuelConsumptionMode` from `Config`. (https://github.com/wasmi-labs/wasmi/pull/877)
    - `FuelConsumptionMode` was required to differentiate between lazy and eager fuel consumption.
      This was necessary due to how lazy fuel consumption was implemented in that it would pre-charge
      for instruction execution were the exact amount of required fuel was not possible to determine
      at compilation time. Examples are `memory.grow` and `table.copy` instructions. The linked PR
      improved lazy fuel consumption to no longer pre-charge and instead pre-check if the operation
      is going to succeed and only charge fuel in that case.

### Dev. Note

- Added execution fuzzing and differential fuzzing.
    - PRs: https://github.com/wasmi-labs/wasmi/pull/832, https://github.com/wasmi-labs/wasmi/pull/833
    - Both fuzzing strategies are applied on each commit in our CI pipeline.
- Updated CI jobs to use `dtolnay/rust-toolchain` instead of `actions-rs` because the latter was deprecated. (https://github.com/wasmi-labs/wasmi/pull/842)

## `0.31.0` - 2023-07-31

### Added

- Added `ResourceLimiter` API known from Wasmtime. (https://github.com/wasmi-labs/wasmi/pull/737)
  - This API allows to limit growable Wasm resources such as Wasm tables and linear memories.
  - Special thanks to [Graydon Hoare](https://github.com/graydon) for contributing this feature!

### Fixes

- Fixed a bug were `Module::len_globals` internal API returned length of linear memories instead. (https://github.com/wasmi-labs/wasmi/pull/741)

### Changed

- Removed `intx` crate dependency. (https://github.com/wasmi-labs/wasmi/pull/727)
  - The dependence on the `intx` crate was accidental and not really required at any time.
- Optimized `f64.const` instructions for `f64` constant values that can losslessly be encoded as 32-bit `f32` value. (https://github.com/wasmi-labs/wasmi/pull/746)

### Dev. Note

- We now publish and record graphs of benchmarks over time. (https://github.com/wasmi-labs/wasmi/pull/740)
  - This allows Wasmi developers to better inspect performance changes over longer periods of time.
- Updated dev. dependencies:
  - `criterion 0.4.0` -> `0.5.0`
  - `wast 0.52.0` -> `0.62.0`

## `0.30.0` - 2023-05-28

### Changed

- Optimized Wasmi bytecode memory consumption. (https://github.com/wasmi-labs/wasmi/pull/718)
  - This reduced the memory consumption of Wasmi bytecode by organizing the instructions
    into so-called instruction words, effectively reducing the amount of bytes required per
    Wasmi instruction 16 bytes to 8 bytes.
    There was an experiment with 4 bytes but experiments confirmed that 8 bytes per instruction
    word was the sweetspot for Wasmi execution and translation performance.
  - This did not affect execution performance too much but we saw performance improvements
    for translation from Wasm to Wasmi bytecode by roughly 15-20%.
- Optimized `call` and `return_call` for Wasm module internal calls. (https://github.com/wasmi-labs/wasmi/pull/724)
  - Wasmi bytecode now differentiates between calls to Wasm module internal functions
    and imported functions which allows the Wasmi bytecode executor to perform the common
    internal calls more efficiently.
  - This led to an execution performance improvement across the board but especially for
    call intense workloads of up to 30% in some test cases.

## `0.29.0` - 2023-03-20

### Added

- Added support for `extended-const` Wasm proposal. (https://github.com/wasmi-labs/wasmi/pull/707)
- Added fuel consumption modes. (https://github.com/wasmi-labs/wasmi/pull/706)
  - This allows eager and lazy fuel consumption modes to be used which
    mainly affects bulk operations such as `table.copy` and `memory.grow`.
    Eager fuel consumption always consumes fuel before a bulk operation for the
    total amount independent of success or failure of the operation whereras
    lazy fuel consumption only consumes fuel for successful executions.

### Changed

- Normalize fuel costs of all instructions. (https://github.com/wasmi-labs/wasmi/pull/705)
  - With this change most instructions cost roughly 1 fuel upon execution.
    This is more similar to how Wasmtime deals with fuel metered instruction costs.
    Before this change Wasmi tried to have fuel costs that more closely mirror
    the computation intensity of the respective instruction according to benchmarks.

## `0.28.0` - 2023-03-01

### Added

- Added support for the `tail-call` Wasm proposal. (https://github.com/wasmi-labs/wasmi/pull/683)
- Added support for `Linker` defined host functions. (https://github.com/wasmi-labs/wasmi/pull/692)
  - Apparently this PR introduced some performance wins for the Wasm target according to our tests.
    This information shall be taken with a grain of salt since we are not sure why those performance
    improvement occured since the PR's functionality is orthogonal to Wasm engine performance.
  - Required precursor refactoring PR: https://github.com/wasmi-labs/wasmi/pull/681

[`tail-call`]: https://github.com/WebAssembly/tail-call

### Changed

- The `wasmi_wasi` crate now more closely mirrors the `wasmtime_wasi` crate API. (https://github.com/wasmi-labs/wasmi/pull/700)

### Internal

- Refactor the Wasmi Wasm engine to handle Wasm calls and returns in its core. [(#694)]
  - This improved performance of Wasm function calls significantly at the cost of host function call performance.
  - Also this seemed to have impacts Wasm target performance quite positively, too.
- The `Store` now handles Wasm functions and host functions separately. (https://github.com/wasmi-labs/wasmi/pull/686)
  - This allows to store Wasm functions into the `StoreInner` type which was an important
    step towards the major refactoring in [(#694)]
  - It was expected that host function call performance would degrade by this PR but our tests
    actually showed that the opposite was true and Wasm target performance was improved overall.
- Introduce `ValueStackPtr` abstraction for the Wasmi engine core. (https://github.com/wasmi-labs/wasmi/pull/688)
  - This change significantly improved performance especially on the Wasm target according to our tests.
- Optimize `memory.{load,store}` when reading or writing single bytes. (https://github.com/wasmi-labs/wasmi/pull/689)
  - The performance wins were more modest than we hoped but still measurable.
- Use `StoreContextMut<T>` instead of `impl AsContextMut` in the Wasmi engine core. (https://github.com/wasmi-labs/wasmi/pull/685)
  - This is a simple refactoring with the goal to make the Rust compiler have a simpler job at
    optimizing certain functions in the engine's inner workings since `StoreContextMut` provides
    more information to the compiler.

[(#694)]: https://github.com/wasmi-labs/wasmi/pull/694

## `0.27.0` - 2023-02-14

### Added

- Added support for fuel metering in the Wasmi CLI. (https://github.com/wasmi-labs/wasmi/pull/679)
  - Users can now specify an amount of fuel via `--fuel N` to commit for the execution.
    Upon success the Wasmi CLI will display the total amount of consumed and remaining fuel.

### Fixed

- Fixed a bug that Wasmi CLI did not preserve the WASI exit status. (https://github.com/wasmi-labs/wasmi/pull/677)
  - Thanks to [YAMAMOTO Takashi @yamt](https://github.com/yamt) for reporting the issue.
- The Wasmi CLI now properly displays exported functions if `--invoke x` was provided and `x` was not found. (https://github.com/wasmi-labs/wasmi/pull/678)
- Applied minor fixes to `Config` docs. (https://github.com/wasmi-labs/wasmi/pull/673)

### Changed

- Defer charging fuel for costly bulk `memory` and bulk `table` operations. (https://github.com/wasmi-labs/wasmi/pull/676)
  - Note that the check to assert that enough fuel is provided for these costly
    operation is still happening before the actual computation and only the charging
    is deferred to after a successful run. The reason behind this is that all the affected
    operations fail fast and therefore should not cost lots of fuel in case of failure.

## `0.26.1` - 2023-02-13

### Fixed

- Fixed a bug where resuming a resumable function from a host function with more outputs than
  inputs could lead to incorrect behavior or runtime panics. (https://github.com/wasmi-labs/wasmi/pull/671)
    - Thanks to [Pierre Krieger (tomaka)](https://github.com/tomaka) for reporting and crafting an initial minimal test case.

## `0.26.0` - 2023-02-11

### Added

- Wasmi CLI: Add WASI support. (https://github.com/wasmi-labs/wasmi/pull/597)
  - Big shoutout to [Onigbinde Oluwamuyiwa Elijah](https://github.com/OLUWAMUYIWA) for contributing this to Wasmi!
- Add built-in support for fuel metering. (https://github.com/wasmi-labs/wasmi/pull/653)
  - This allows to control the runtime of Wasm executions in a deterministic fasion
    effectively avoiding the halting problem by charging for executed instructions.
    Not using the feature will not affect the execution efficiency of Wasmi for users.
- Add `Pages::checked_sub` method. (https://github.com/wasmi-labs/wasmi/pull/660)
- Add `Func::new` constructor. (https://github.com/wasmi-labs/wasmi/pull/662)
  - This allows to create `Func` instances from closures without statically known types.

### Changed

- Update to `wasmparser-nostd` version `0.100.1`. (https://github.com/wasmi-labs/wasmi/pull/666)

### Internal

- Clean up and reorganization of the `wasmi_cli` crate. (https://github.com/wasmi-labs/wasmi/pull/655)
- Refactoring of internal host call API. (https://github.com/wasmi-labs/wasmi/pull/664)

## `0.25.0` - 2023-02-04

### Added

- Added `Config::floats` option to enable or disable Wasm float operators during Wasm validation.
- `Trap::downcast_mut` and `Trap::downcast` methods. (https://github.com/wasmi-labs/wasmi/pull/650)
  - This helps users to downcast into `T: HostError`.
- Added `WasmType` impls for `FuncRef` and `ExternRef` types. (https://github.com/wasmi-labs/wasmi/pull/642)
  - This allows `FuncRef` and `ExternRef` instances to be used in `TypedFunc` parameters and results.

### Removed

- Removed from `From` impls from `wasmparser-nostd` types to Wasmi types.
  - For example `From<wasmparser::FuncType> for wasmi::FuncType` got removed.

### Changed

- Update the `wasmparser-nostd` dependency from version `0.91.0` to `0.99.0`. (https://github.com/wasmi-labs/wasmi/pull/640)
- The `Trap` type is no longer `Clone`. (https://github.com/wasmi-labs/wasmi/pull/650)

### Internal

- Resolved plenty of technical debt and improved structure of the Wasmi crate.
  - PRs: https://github.com/wasmi-labs/wasmi/pull/648, https://github.com/wasmi-labs/wasmi/pull/647, https://github.com/wasmi-labs/wasmi/pull/646, https://github.com/wasmi-labs/wasmi/pull/645, https://github.com/wasmi-labs/wasmi/pull/644, https://github.com/wasmi-labs/wasmi/pull/641

## `0.24.0` - 2023-01-31

### Added

- Added support for the [`bulk-memory`] Wasm proposal. (https://github.com/wasmi-labs/wasmi/pull/628)
- Added support for the [`reference-types`] Wasm proposal. (https://github.com/wasmi-labs/wasmi/pull/635)
- Added `ValueType::{is_ref, is_num`} methods. (https://github.com/wasmi-labs/wasmi/pull/635)
- Added `Value::{i32, i64, f32, f64, externref, funcref}` accessor methods to `Value`.

[`bulk-memory`]: https://github.com/WebAssembly/bulk-memory-operations
[`reference-types`]: https://github.com/WebAssembly/reference-types

### Fixed

- Fix a bug with `Table` and `Memory` imports not respecting the current size. (https://github.com/wasmi-labs/wasmi/pull/635)
  - This sometimes led to the problem that valid `Table` and `Memory` imports
    could incorrectly be rejected for having an invalid size for the subtype check.
  - This has been fixed as part of the [`reference-types`] Wasm proposal implementation.

### Changed

- Use more references in places to provide the compiler with more optimization opportunities. (https://github.com/wasmi-labs/wasmi/pull/634)
  - This led to a speed-up across the board for Wasm targets of about 15-20%.
- Move the `Value` type from `wasmi_core` to Wasmi. (https://github.com/wasmi-labs/wasmi/pull/636)
  - This change was necessary in order to support the [`reference-types`] Wasm proposal.
- There has been some consequences from implementing the [`reference-types`] Wasm proposal which are listed below:
  - The `Value` type no longer implements `Copy` and `PartialEq`.
  - The `From<&Value> for UntypedValue` impl has been removed.
  - Remove some `From` impls for `Value`.
  - Moved some `Display` impls for types like `FuncType` and `Value` to the `wasmi_cli` crate.
  - Remove the `try_into` API from the `Value` type.
    - Users should use the new accessor methods as in the Wasmtime API.

### Internal

- Update `wast` dependency from version `0.44` to `0.52`. (https://github.com/wasmi-labs/wasmi/pull/632)
- Update the Wasm spec testsuite to the most recent commit: `3a04b2cf9`
- Improve error reporting for the internal Wasm spec testsuite runner.
  - It will now show proper span information in many more cases.

## `0.23.0` - 2023-01-19

> **Note:** This is the Wasmtime API Compatibility update.

### Added

- Add `Module::get_export` method. (https://github.com/wasmi-labs/wasmi/pull/617)

### Changed

- Removed `ModuleError` export from crate root. (https://github.com/wasmi-labs/wasmi/pull/618)
  - Now `ModuleError` is exported from `crate::errors` just like all the other error types.
- Refactor and cleanup traits underlying to `IntoFunc`. (https://github.com/wasmi-labs/wasmi/pull/620)
  - This is only the first step in moving closer to the Wasmtime API traits.
- Mirror Wasmtime API more closely. (https://github.com/wasmi-labs/wasmi/pull/615, https://github.com/wasmi-labs/wasmi/pull/616)
  - Renamed `Caller::host_data` method to `Caller::data`.
  - Renamed `Caller::host_data_mut` method to `Caller::data_mut`.
  - Add `Extern::ty` method and the `ExternType` type.
  - Rename `ExportItem` to `ExportType`:
    - Rename the `ExportItem::kind` method to `ty` and return `ExternType` instead of `ExportItemKind`.
    - Remove the no longer used `ExportItemKind` entirely.
  - The `ExportsIter` now yields items of the new type `Export` instead of pairs of `(&str, Extern)`.
  - Rename `ModuleImport` to `ImportType`.
    - Rename `ImportType::item_type` to `ty`.
    - Rename `ImportType::field` to `name`.
    - Properly forward `&str` lifetimes in `ImportType::{module, name}`.
    - Replace `ModuleImportType` by `ExternType`.
  - Add new convenience methods to `Instance`:
    - `Instance::get_func`
    - `Instance::get_typed_func`
    - `Instance::get_global`
    - `Instance::get_table`
    - `Instance::get_memory`
  - Rename getters for querying types of runtime objects:
    - `Func::func_type` => `Func::ty`
    - `Global::global_type` => `Global::ty`
    - `Table::table_type` => `Table::ty`
    - `Memory::memory_type` => `Memory::ty`
    - `Value::value_type` => `Value::ty`
  - Remove `Global::value_type` getter.
    - Use `global.ty().content()` instead.
  - Remove `Global::is_mutable` getter.
    - Use `global.ty().mutability().is_mut()` instead.
  - Rename `Mutability::Mutable` to `Var`.
  - Add `Mutability::is_mut` getter.
    - While this API is not included in Wasmtime it is a useful convenience method.
  - Rename `TableType::initial` method to `minimum`.
  - Rename `Table::len` method to `size`.
  - `Table` and `TableType` now operate on `u32` instead of `usize` just like in Wasmtime.
    - This affects `Table::{new, size, set, get, grow}` methods and `TableType::{new, minimum, maximum}` methods and their users.

## `0.22.0` - 2023-01-16

### Added

- Add missing `TypedFunc::call_resumable` API. (https://github.com/wasmi-labs/wasmi/pull/605)
  - So far resumable calls were only available for the `Func` type.
    However, there was no technical reason why it was not implemented
    for `TypedFunc` so this mirrored API now exists.
  - This also cleans up rough edges with the `Func::call_resumable` API.

### Changed

- Clean up the `wasmi_core` crate API. (https://github.com/wasmi-labs/wasmi/pull/607, https://github.com/wasmi-labs/wasmi/pull/608, https://github.com/wasmi-labs/wasmi/pull/609)
  - This removes plenty of traits from the public interface of the crate
    which greatly simplifies the API surface for users.
  - The `UntypedValue` type gained some new methods to replace functionality
    that was provided in parts by the removed traits.
- The Wasmi crate now follows the Wasmtime API a bit more closely. (https://github.com/wasmi-labs/wasmi/pull/613)
  - `StoreContext` new methods:
    - `fn engine(&self) -> &Engine`
    - `fn data(&self) -> &T` 
  - `StoreContextMut` new methods:
    - `fn engine(&self) -> &Engine`
    - `fn data(&self) -> &T` 
    - `fn data_mut(&mut self) -> &mut T`
  - Renamed `Store::state` method to `Store::data`.
  - Renamed `Store::state_mut` method to `Store::data_mut`.
  - Renamed `Store::into_state` method to `Store::into_data`.
### Internal

- The `Store` and `Engine` types are better decoupled from their generic parts. (https://github.com/wasmi-labs/wasmi/pull/610, https://github.com/wasmi-labs/wasmi/pull/611)
  - This might reduce binary bloat and may have positive effects on the performance.
    In fact we measured significant performance improvements on the Wasm target.

## `0.21.0` - 2023-01-04

### Added

- Add support for resumable function calls. (https://github.com/wasmi-labs/wasmi/pull/598)
  - This feature allows to resume a function call upon encountering a host trap.
- Add support for concurrently running function executions using a single Wasmi engine.
  - This feature also allows to call Wasm functions from host functions. (https://github.com/wasmi-labs/wasmi/pull/590)
- Add initial naive WASI support for Wasmi using the new `wasmi_wasi` crate. (https://github.com/wasmi-labs/wasmi/pull/557)
  - Special thanks to [Onigbinde Oluwamuyiwa Elijah](https://github.com/OLUWAMUYIWA) for carrying the WASI support efforts!
  - Also thanks to [Yuyi Wang](https://github.com/Berrysoft) for testing and improving initial WASI support. (https://github.com/wasmi-labs/wasmi/pull/592, https://github.com/wasmi-labs/wasmi/pull/571, https://github.com/wasmi-labs/wasmi/pull/568)
  - **Note:** There is ongoing work to integrate WASI support in `wasmi_cli` so that the Wasmi CLI will then
              be able to execute arbitrary `wasm-wasi` files out of the box in the future.
- Add `Module::imports` that allows to query Wasm module imports. (https://github.com/wasmi-labs/wasmi/pull/573, https://github.com/wasmi-labs/wasmi/pull/583)

### Fixed

- Fix a bug that imported linear memories and tables were initialized twice upon instantiation. (https://github.com/wasmi-labs/wasmi/pull/593)
- The Wasmi CLI now properly hints for file path arguments. (https://github.com/wasmi-labs/wasmi/pull/596)

### Changed

- The `wasmi::Trap` type is now more similar to Wasmtime's `Trap` type. (https://github.com/wasmi-labs/wasmi/pull/559)
- The `wasmi::Store` type is now `Send` and `Sync` as intended. (https://github.com/wasmi-labs/wasmi/pull/566)
- The Wasmi CLI now prints exported functions names if the function name CLI argument is missing. (https://github.com/wasmi-labs/wasmi/pull/579)
- Improve feedback when running a Wasm module without exported function using Wasmi CLI. (https://github.com/wasmi-labs/wasmi/pull/584)

## `0.20.0` - 2022-11-04

### Added

- Contribution documentation about fuzz testing. (https://github.com/wasmi-labs/wasmi/pull/529)

### Removed

- Removed some deprecated functions in the `wasmi_core` crate. (https://github.com/wasmi-labs/wasmi/pull/545)

### Fixed

- Fixed a critical performance regression introduced in Rust 1.65. (https://github.com/wasmi-labs/wasmi/pull/518)
  - While the PR's main job was to clean up some code it was found out that it
    also fixes a critical performance regression introduced in Rust 1.65.
  - You can read more about this performance regression [in this thread](https://github.com/rust-lang/rust/issues/102952).

### Changed

- Fixed handling of edge cases with respect to Wasm linear memory. (https://github.com/wasmi-labs/wasmi/pull/449)
  - This allows for Wasmi to properly setup and use linear memory instances of up to 4GB.
- Optimize and improve Wasm instantiation. (https://github.com/wasmi-labs/wasmi/pull/531)
- Optimize `global.get` of immutable non-imported globals. (https://github.com/wasmi-labs/wasmi/pull/533)
  - Also added a benchmark test for this. (https://github.com/wasmi-labs/wasmi/pull/532)

### Internal

- Implemented miscellaneous improvements to our CI system.
  - https://github.com/wasmi-labs/wasmi/pull/539 (and more)
- Miscellaneous clean ups in `wasmi_core` and Wasmi's executor.
  - https://github.com/wasmi-labs/wasmi/pull/542 https://github.com/wasmi-labs/wasmi/pull/541
  https://github.com/wasmi-labs/wasmi/pull/508 https://github.com/wasmi-labs/wasmi/pull/543

## `0.19.0` - 2022-10-20

### Fixed

- Fixed a potential undefined behavior as reported by the `miri` tool
  with respect to its experimental stacked borrows. (https://github.com/wasmi-labs/wasmi/pull/524)

### Changed

- Optimized Wasm to Wasmi translation phase by removing unnecessary Wasm
  validation type checks. (https://github.com/wasmi-labs/wasmi/pull/527)
    - Speedups were in the range of 15%.
- `Linker::instantiate` now takes `&self` instead of `&mut self`. (https://github.com/wasmi-labs/wasmi/pull/512)
  - This allows users to easily predefine a linker and reused its definitions
    as shared resource.
- Fixed a bug were `Caller::new` was public. (https://github.com/wasmi-labs/wasmi/pull/514)
  - It is now a private method as it was meant to be.
- Optimized `TypedFunc::call` at slight cost of `Func::call`. (https://github.com/wasmi-labs/wasmi/pull/522)
  - For many parameters and return values the measured improvements are in the range of 25%.
    Note that this is only significant for a large amount of host to Wasm calls of small functions.

### Internal

- Added new benchmarks and cleaned up benchmarking code in general.
  - https://github.com/wasmi-labs/wasmi/pull/525
  https://github.com/wasmi-labs/wasmi/pull/526
  https://github.com/wasmi-labs/wasmi/pull/521
- Add `miri` testing to Wasmi CI (https://github.com/wasmi-labs/wasmi/pull/523)

## `0.18.1` - 2022-10-13

### Changed

- Optimize for common cases for branch and return instructions.
  (https://github.com/wasmi-labs/wasmi/pull/493)
    - This led to up to 10% performance improvement according to our benchmarks
      in some cases.
- Removed extraneous `S: impl AsContext` generic parameter from `Func::typed` method.
- Make `IntoFunc`, `WasmType` and `WasmRet` traits publicly available.
- Add missing impl for `WasmRet` for `Result<T, Trap> where T: WasmType`.
    - Without this impl it was impossible to provide closures to `Func::wrap`
      that returned `Result<T, Trap>` where `T: WasmType`, only `Result<(), Trap>`
      or `Result<(T,), Trap>` was possible before.

### Internal

- Added `wasmi_arena` crate which defines all internally used arena data structures.
  (https://github.com/wasmi-labs/wasmi/pull/502)
- Update to `clap 4.0` in `wasmi_cli`. (https://github.com/wasmi-labs/wasmi/pull/498)
- Many more improvements to our internal benchmarking CI.
  (https://github.com/wasmi-labs/wasmi/pull/494, https://github.com/wasmi-labs/wasmi/pull/501,
  https://github.com/wasmi-labs/wasmi/pull/506, https://github.com/wasmi-labs/wasmi/pull/509)

## `0.18.0` - 2022-10-02

### Added

- Added Contibution Guidelines and Code of Conduct to the repository. (https://github.com/wasmi-labs/wasmi/pull/485)

### Changed

- Optimized instruction dispatch in the Wasmi interpreter.
  (https://github.com/wasmi-labs/wasmi/pull/478, https://github.com/wasmi-labs/wasmi/pull/482)
  - This yielded combined speed-ups of ~20% across the board.
  - As a side effect we also refactored the way we compute branching offsets
    at Wasm module compilation time which improved performance of Wasm module
    compilation by roughly 5%.

### Internal

- Our CI now also benchmarks Wasmi when ran inside Wasmtime as Wasm.
  (https://github.com/wasmi-labs/wasmi/pull/483, https://github.com/wasmi-labs/wasmi/pull/487)
  - This allows us to optimize Wasmi towards Wasm performance more easily in the future.

## `0.17.0` - 2022-09-23

### Added

- Added `Memory::data_and_store_mut` API inspired by Wasmtime's API. (https://github.com/wasmi-labs/wasmi/pull/448)

### Changed

- Updated `wasmparser-nostd` dependency from `0.90.0` to `0.91.0`.
    - This improved performance of Wasm module compilation by ~10%.
- Updated `wasmi_core` from `0.3.0` to `0.4.0`.
- Optimized execution of several Wasm float to int conversion instructions. (https://github.com/wasmi-labs/wasmi/pull/439)
    - We measured a performance improvement of 6000% or in other words those
      instructions are now 60 times faster than before.
    - This allowed us to remove the big `num-rational` dependency from `wasmi_core`
      for some nice speed-ups in compilation time of Wasmi itself.
- Optimized `global.get` and `global.set` Wasm instruction execution. (https://github.com/wasmi-labs/wasmi/pull/427)
    - This improved performance of those instructions by up to 17%.
- Optimized Wasm value stack emulation. (https://github.com/wasmi-labs/wasmi/pull/459)
    - This improved performance of compute intense workloads by up to 23%.

### Internal

- Added automated continuous benchmarking to Wasmi. (https://github.com/wasmi-labs/wasmi/pull/422)
    - This allows us to have a more consistent overview over the performance of Wasmi.
- Updated `criterion` benchmarking framework to version `0.4.0`.
- Reuse allocations during Wasm validation and translation:
     - Wasm validation and translation combined. (https://github.com/wasmi-labs/wasmi/pull/462)
     - Wasm `br_table` translations. (https://github.com/wasmi-labs/wasmi/pull/440)
- Enabled more useful `clippy` lints for Wasmi and `wasmi_core`. (https://github.com/wasmi-labs/wasmi/pull/438)
- Reorganized the Wasmi workspace. (https://github.com/wasmi-labs/wasmi/pull/466)

## `0.16.0` - 2022-08-30

### Changed

- Update `wasmparser-nostd` dependency from version `0.83.0` -> `0.90.0`.
  [**Link:**](https://github.com/wasmi-labs/wasmi/commit/e9b0463817e277cd9daccca7e66e52e4fd147d8e)
    - This significantly improved Wasmi's Wasm parsing, validation and
      Wasm to Wasmi bytecode translation performance.

### Internal

- Transition to the new `wasmparser::VisitOperator` API.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/225c8224729661ea091e650e3278c4980bd1d405)
    - This again significantly improved Wasmi's Wasm parsing, validation and
      Wasm to Wasmi bytecode translation performance by avoiding many
      unnecessary unpredictable branches in the process.

## `0.15.0` - 2022-08-22

### Fixed

- Fixed bugs found during fuzzing the translation phase of Wasmi.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/43d7037745a266ece2baccd9e78f7d983dacbb93)
- Fix `Read` trait implementation for `no_std` compilations.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/baab359de955240fbb9c89ebbc369d7a6e6d8569)

### Changed

- Update to `wasmi_core` version `0.3.0`.
- Changed API of `wasmi::Config` in order to better reflect the API of
  `wasmtime::Config`.
- Refactor `Trap` type to be of pointer size which resulted in significant
  performance wins across the board especially for call intense work loads.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/4a5d113a11a0f0020491c2cc08dd195a184256f0)

### Removed

- Removed support for virtual memory based Wasm linear memory.
  We decided to remove support since benchmarks showed that our current
  implementation actually regresses performance compared to our naive
  `Vec` based implementation.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/10f8780a49b8cc8d8719e2b74089bf6848b8f982)

### Internal

- The `wasmi::Engine` now caches the bytes of the default linear memory for
  performance wins in `memory.store` and `memory.load` intense work loads.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/c0df344e970bcdd4c6ce25f64265c854a1239220)
- The Wasmi engine internals have been reorganized and modernised to improve
  performance on function call intense work loads. This resulted in performance
  improvements across the board.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/d789570b51effb3a0c397c2d4ea1dc03c5d76918)
- The Wasm to Wasmi bytecode translation now properly reuses heap allocations
  across function translation units which improved translation performance by
  roughly 10%.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/71a913fc508841b3b7f799c8e4406e1e48feb046)
- Optimized the Wasmi engine Wasm value stack implementation for significant
  performance wins across the board.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/3886d9190e89d44a701ad5cbbda0c7457feba510)
- Shrunk size of some internal identifier types for minor performance wins.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/3d544b82a5089ae4331024b1e6762dcb48a02898)
- Added initial naive fuzz testing for Wasm parsing, validation and Wasm to
  Wasmi bytecode translation.
  [**Link**](https://github.com/wasmi-labs/wasmi/commit/4d1f2ad6cbf07e61656185101bbd0bd5a941335f)

## `0.14.0` - 2022-07-26

### Added

- Added support for the following Wasm proposals:

    - [Import and export of mutable globals](https://github.com/WebAssembly/mutable-global)
    - [Non-trapping float-to-int conversions](https://github.com/WebAssembly/nontrapping-float-to-int-conversions)
    - [Sign-extension operators](https://github.com/WebAssembly/sign-extension-ops)
    - [Multi-value](https://github.com/WebAssembly/multi-value)

  We plan to support more Wasm proposals in the future.

### Changed

- Wasmi has been entirely redesigned and reimplemented.
  This work resulted in an entirely new API that is heavily inspired by
  the [Wasmtime API](https://docs.rs/wasmtime/0.39.1/wasmtime/),
  a brand new Wasm execution engine that performs roughly 30-40%
  better than the previous engine according to our benchmarks,
  the support of many Wasm proposals and Wasm parsing and validation
  using the battle tested [`wasmparser`](https://crates.io/crates/wasmparser)
  crate by the BytecodeAlliance.

  The new Wasmi design allows to reuse the Wasm execution engine
  resources instead of spinning up a new Wasm execution engine for every
  function call.

  **Note:** If you plan to use Wasmi it is of critical importance
  to compile Wasmi using the following Cargo `profile` settings:

  ```toml
  [profile.release]
  lto = "fat"
  codegen-units = 1
  ```

  If you do not use these profile settings you might risk regressing
  performance of Wasmi by up to 400%. You can read more about this
  issue [here](https://github.com/wasmi-labs/wasmi/issues/339).

### Removed

- Removed support for resuming function execution.
  We may consider to add this feature back into the new engine.
  If you are a user of Wasmi and want this feature please feel
  free to [open an issue](https://github.com/wasmi-labs/wasmi/issues)
  and provide us with your use case.

## `0.13.2` - 2022-09-20

### Fixed

- Support allocating 4GB of memory (https://github.com/wasmi-labs/wasmi/pull/452)

## `0.13.1` - 2022-09-20

**Note:** Yanked because of missing `wasmi_core` bump.

## `0.13.0` - 2022-07-25

**Note:** This is the last major release of the legacy Wasmi engine.
          Future releases are using the new Wasm execution engines
          that are currently in development.
          We may consider to publish new major versions of this Wasm engine
          as `wasmi-legacy` crate.

### Changed

- Update dependency: `wasmi-validation v0.4.2 -> v0.5.0`

## `0.12.0` - 2022-07-24

### Changed

- Wasmi now depends on the [`wasmi_core`](https://crates.io/crates/wasmi_core) crate.
- Deprecated `RuntimeValue::decode_{f32,f64}` methods.
    - **Reason**: These methods expose details about the `F32` and `F64` types.
                  The `RuntimeValue` type provides `from_bits` methods for similar purposes.
    - **Replacement:** Replace those deprecated methods with `F{32,64}::from_bits().into()` respectively.
- Refactor traps in Wasmi: [PR](https://github.com/wasmi-labs/wasmi/commit/cd59462bc946a52a7e3e4db491ac6675e3a2f53f)
    - This change also renames `TrapKind` to `TrapCode`.
    - The Wasmi crate now properly reuses the `TrapCode` definitions from the `wasmi_core` crate.
- Updated dependency:
    - `parity-wasm v0.42 -> v0.45`
    - `memory_units v0.3.0 -> v0.4.0`

### Internal

- Rename `RuntimeValue` to `Value` internally.
- Now uses `wat` crate dependency instead of `wabt` for reading `.wat` files in tests.
- Updated dev-dependencies:
    - `assert_matches: v1.1 -> v1.5`
    - `rand 0.4.2 -> 0.8.2`
- Fix some `clippy` warnings.

## `0.11.0` - 2022-01-06

### Fixed

- Make Wasmi traps more conformant with the Wasm specification. (https://github.com/wasmi-labs/wasmi/pull/300)
- Fixed a bug in `{f32, f64}_copysign` implementations. (https://github.com/wasmi-labs/wasmi/pull/293)
- Fixed a bug in `{f32, f64}_{min, max}` implementations. (https://github.com/wasmi-labs/wasmi/pull/295)

### Changed

- Optimized Wasm to host calls. (https://github.com/wasmi-labs/wasmi/pull/291)
    - In some artificial benchmarks we saw improvements of up to 42%!
- Introduce a more efficient `LittleEndianConvert` trait. (https://github.com/wasmi-labs/wasmi/pull/290)

### Internal

- Refactor and clean up benchmarking code and added more benchmarks.
    - https://github.com/wasmi-labs/wasmi/pull/299
    - https://github.com/wasmi-labs/wasmi/pull/298
- Apply some clippy suggestions with respect ot `#[must_use]`. (https://github.com/wasmi-labs/wasmi/pull/288)
- Improve Rust code formatting of imports.
- Improve debug impl of `ValueStack` so that only the live parts are printed.

## `0.10.0` - 2021-12-14

### Added

- Support for virtual memory usage on Windows 64-bit platforms.
    - Technically we now support the same set of platforms as the `region` crate does:
      https://github.com/darfink/region-rs#platforms

### Changed

- The Wasmi and `wasmi-validation` crates now both use Rust edition 2021.
- The `README` now better teaches how to test and benchmark the crate.
- Updated `num-rational` from version `0.2.2` -> `0.4.0`.

### Deprecated

- Deprecated `MemoryInstance::get` method.
    - Users are recommended to use `MemoryInstance::get_value` or `MemoryInstance::get_into`
      methods instead.

### Removed

- Removed support for virtual memory on 32-bit platforms.
    - Note that the existing support was supposedly not more efficient than the `Vec`
      based fallback implementation anyways due to technical design.
- Removed the `core` crate feature that previously has been required for `no_std` builds.
    - Now users only have to specify `--no-default-features` for a `no_std` build.

### Internal

- Fully deploy GitHub Actions CI and remove deprecated Travis based CI. Added CI jobs for:
    - Testing on Linux, MacOS and Windows
    - Checking docs and dead links in docs.
    - Audit crate dependencies for vulnerabilities.
    - Check Wasm builds.
    - File test coverage reports to codecov.io.

## `0.9.1` - 2021-09-23

### Changed

- Added possibility to forward `reduced_stack_buffers` crate feature to `parity-wasm` crate.

### Internal

- Added a default `rustfmt.toml` configuration file.
- Fixed some warnings associated to Rust edition 2021.
    - Note: The crate itself remains in Rust edition 2018.

## `0.9.0` - 2021-05-27

### Changed

- Updated `parity-wasm` from verion `0.41` to `0.42`.
- Bumped `wasmi-validation` from version `0.3.1` to `0.4.0`.
