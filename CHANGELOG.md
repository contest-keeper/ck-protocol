# Changelog

## [0.4.0](https://github.com/contest-keeper/ck-protocol/compare/v0.3.0...v0.4.0) (2026-11-19)


### Features

* add the first boot enrolment config ([293131b](https://github.com/contest-keeper/ck-protocol/commit/293131bab0de9c55d508d04f446b3803420f1ec0))
* add the signed screen unlock action ([e2c72b0](https://github.com/contest-keeper/ck-protocol/commit/e2c72b0378696447049a0dbe9df29e87bca2b46d))

## [0.3.0](https://github.com/contest-keeper/ck-protocol/compare/v0.2.0...v0.3.0) (2026-11-19)


### Features

* read and write files on the ckconfig partition in ck-image ([#35](https://github.com/contest-keeper/ck-protocol/issues/35)) ([407dc1c](https://github.com/contest-keeper/ck-protocol/commit/407dc1c61070b5c37c6c32b3e3decc317e7e31d4))

## [0.2.0](https://github.com/contest-keeper/ck-protocol/compare/v0.1.0...v0.2.0) (2026-11-18)


### Features

* add GPT reading, in-place EFI file access and image copying ([#7](https://github.com/contest-keeper/ck-protocol/issues/7)) ([9bca200](https://github.com/contest-keeper/ck-protocol/commit/9bca2004f31e81331fb567678d8369bbe6831687))
* add hub commands, signed envelope and ack event ([#24](https://github.com/contest-keeper/ck-protocol/issues/24)) ([616e4e1](https://github.com/contest-keeper/ck-protocol/commit/616e4e147afc7e77673b885cf35b861d8b5171f5))
* add settings, stick config and heartbeat types ([#5](https://github.com/contest-keeper/ck-protocol/issues/5)) ([eba2311](https://github.com/contest-keeper/ck-protocol/commit/eba2311c553f218f7dd44ab650ee4d218925ce36))
* add stick token generation and HMAC signing ([#6](https://github.com/contest-keeper/ck-protocol/issues/6)) ([97e0627](https://github.com/contest-keeper/ck-protocol/commit/97e0627323a3ce758a80bf28c090798feccda1da))
* add the admin password hash setting ([#25](https://github.com/contest-keeper/ck-protocol/issues/25)) ([5b46dc5](https://github.com/contest-keeper/ck-protocol/commit/5b46dc588774725ca3a185b252d8023a0d01fbb3))
* add the block bluetooth setting ([#20](https://github.com/contest-keeper/ck-protocol/issues/20)) ([2eb5fa2](https://github.com/contest-keeper/ck-protocol/commit/2eb5fa2a38f28f9e92fa9f6687bb962d8d12b3cb))
* add the event envelope and the chain check ([#11](https://github.com/contest-keeper/ck-protocol/issues/11)) ([2503ab9](https://github.com/contest-keeper/ck-protocol/commit/2503ab9fbb3be77089307adb5cbf9ff83469cd13))
* add the machine number and the computer name to the stick config ([#10](https://github.com/contest-keeper/ck-protocol/issues/10)) ([abf69fa](https://github.com/contest-keeper/ck-protocol/commit/abf69fab780bb3d12ab73b18e3c4c99593a9aa5f))
* add the network event kind ([#14](https://github.com/contest-keeper/ck-protocol/issues/14)) ([4aa1e0f](https://github.com/contest-keeper/ck-protocol/commit/4aa1e0fcc12b80b30382060351802acbd4231444))
* add the recording upload settings ([#22](https://github.com/contest-keeper/ck-protocol/issues/22)) ([85b714d](https://github.com/contest-keeper/ck-protocol/commit/85b714dab95bb5d03aa140e6db9d87c990443474))
* add the wifi setting with a hidden password ([#21](https://github.com/contest-keeper/ck-protocol/issues/21)) ([691bd74](https://github.com/contest-keeper/ck-protocol/commit/691bd74661c960ebfe8f7139a895347c4e298414))
* define the CKCONFIG config file and schema 2 ([#23](https://github.com/contest-keeper/ck-protocol/issues/23)) ([d5039ad](https://github.com/contest-keeper/ck-protocol/commit/d5039ad2b82749c8d35c21f1dc6202a14a3aa129))


### Bug Fixes

* repair the dot entries fatfs writes so Linux fsck leaves the EFI partition alone ([#9](https://github.com/contest-keeper/ck-protocol/issues/9)) ([e569104](https://github.com/contest-keeper/ck-protocol/commit/e569104d48c803b42dc6148819ece612f8ae88ac))
