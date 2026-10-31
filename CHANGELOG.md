# Changelog

## [0.2.0](https://github.com/ifdotpy/layr-studio/compare/v0.1.0...v0.2.0) (2026-10-31)


### Features

* provide stream bootstrap identity ([654989f](https://github.com/ifdotpy/layr-studio/commit/654989f68df32e9de95ae081720fd550ae3f111f))
* **runtime:** add typed agent permission contract ([d99c229](https://github.com/ifdotpy/layr-studio/commit/d99c229768e417c21b53e68d64d3b52812e64358))
* **web:** accept stream bootstrap sessions ([dfc589b](https://github.com/ifdotpy/layr-studio/commit/dfc589b0eb637fabc59b9830dcab0a6f9d085b3a))
* **web:** add independent screen route selectors ([22ad92a](https://github.com/ifdotpy/layr-studio/commit/22ad92afc73a86e2e6fa92851b98003d3b2c43bc))
* **web:** define chat selection URLs ([263794f](https://github.com/ifdotpy/layr-studio/commit/263794f7c593593d35885fbf6ae018645d36f93f))


### Bug Fixes

* **runtime:** align closed record schemas and fixtures ([f0946ac](https://github.com/ifdotpy/layr-studio/commit/f0946ac68567dd7a9eade0976c0394c42ba223b5))
* **runtime:** authorize stream progress resources ([9417351](https://github.com/ifdotpy/layr-studio/commit/94173518cf22f25b71455b69b482c595a367e948))
* **runtime:** list unsupported Codex permission modes explicitly ([e95b9aa](https://github.com/ifdotpy/layr-studio/commit/e95b9aa3aa51c5229e8e90f449e791c97469a3ad))
* **runtime:** preserve frozen permission receipt compatibility ([9083dee](https://github.com/ifdotpy/layr-studio/commit/9083dee0d967c0a3c3bce9c9b38fd1d5aca67178))
* **runtime:** restore analytics revision schema on upgrade ([231a001](https://github.com/ifdotpy/layr-studio/commit/231a001af502f2cf195b507dfb6575a45a2efb01))
* **web:** avoid replaying preferences on projection ([ff263db](https://github.com/ifdotpy/layr-studio/commit/ff263db32760e5e02738334e570d26026969c1c9))
* **web:** consume read selection on navigation ([1f33233](https://github.com/ifdotpy/layr-studio/commit/1f332330f251e36e0a89f411959d486cbe9c0c64))
* **web:** identify push subscriptions without local storage ([8a435d0](https://github.com/ifdotpy/layr-studio/commit/8a435d025371a2ee5a005b72a272dceb72038392))
* **web:** mark chats read only after selection ([f97f7c4](https://github.com/ifdotpy/layr-studio/commit/f97f7c4b3e5a8a66d75075f3316ab128c5025326))
* **web:** preserve preference edits before projection ([263cd77](https://github.com/ifdotpy/layr-studio/commit/263cd775f3fcce7f3837be2916cd0578b386b681))
* **web:** remove retired draft and upload copies ([6047716](https://github.com/ifdotpy/layr-studio/commit/604771669c84dd2aa713566dd84ba0413ff80439))
* **web:** use streamed discovery snapshots ([a263ae0](https://github.com/ifdotpy/layr-studio/commit/a263ae0eedc38bc5fa6254f9c098d0b556e3a71c))


### Performance

* **tooling:** key gate results on tree content and rerun only failed steps ([#719](https://github.com/ifdotpy/layr-studio/issues/719)) ([826bc85](https://github.com/ifdotpy/layr-studio/commit/826bc858f2fb9d77a86db536a1bdd0e76f38f62f))
* **tooling:** link test dependencies dynamically and merge integration test binaries ([#712](https://github.com/ifdotpy/layr-studio/issues/712)) ([a02389e](https://github.com/ifdotpy/layr-studio/commit/a02389e0d6905551817ae57e0e02b10c89fc1569))


### Reverts

* 661 (provider queue server) from the integration ([8bf41d5](https://github.com/ifdotpy/layr-studio/commit/8bf41d5f93050e4083313a97eca9ae3fd8c775b0))
