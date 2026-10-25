# Avisos de terceros

Parabellum incluye componentes de terceros: la fuente Inter y las librerías de Rust que se compilan dentro del programa. Aquí aparecen con su origen, su licencia y su aviso de copyright.

Cada componente de terceros conserva su propia licencia, y el [LICENSE](LICENSE) del proyecto no cambia sus condiciones.

Versión de Parabellum: 0.40.0 (`src-tauri/Cargo.toml`).

Este documento lo genera `docs/avisos-de-terceros.py` (`python docs/avisos-de-terceros.py`, desde la raíz del repo). No se edita a mano: para actualizarlo tras cambiar dependencias, vuelve a correrlo.

## 1. Componentes que no son de Rust

| Componente | Qué es y dónde está | Licencia | Autor y copyright |
|---|---|---|---|
| Inter (fuente) | Fuente de la interfaz (versión web variable de 100 a 900) y las del panel del juego (versión 4.001, que `src-tauri/src/juego.rs` incluye en el .exe). Ficheros: `ui/fonts/inter-latin.woff2`, `ui/fonts/inter-latin-ext.woff2`, `ui/fonts/OFL.txt`, `src-tauri/juego/fuentes/Inter-Regular.ttf`, `src-tauri/juego/fuentes/Inter-SemiBold.ttf`, `src-tauri/juego/fuentes/OFL.txt`. | SIL Open Font License 1.1 (texto en la sección 4) | Copyright 2016 The Inter Project Authors (https://github.com/rsms/inter) |

Los 18 .jar de los mods (`src-tauri/juego/mods/`) solo contienen código, recursos y metadatos propios de Parabellum (revisado al generar): no llevan librerías de terceros. Minecraft, Fabric y Forge no van incluidos: los pone el propio juego.

## 2. Dependencias de Rust

Son los paquetes que entran en el ejecutable: las dependencias normales (no las de desarrollo ni las de compilación) del paquete `parabellum`, para Windows (`x86_64-pc-windows-msvc`) y macOS (`aarch64-apple-darwin`), los dos objetivos que compila la CI. Se sacaron de `cargo metadata --locked`.

Total: 440 paquetes (412 nombres distintos). Identificadores de licencia distintos en Rust: 15 (licencias y una excepción, LLVM-exception); a ellos se suma la de la fuente Inter (sección 1).

La columna de licencia es la expresión que declara cada paquete tal cual. Algunas no son SPDX estricta (por ejemplo `Apache-2.0 / MIT`, `BSD-3-Clause/MIT`, `MIT/Apache-2.0`, `Unlicense/MIT`).

| Nombre | Versión | Licencia | Repositorio |
|---|---|---|---|
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | https://github.com/oyvindln/adler2 |
| aead | 0.5.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| aes | 0.8.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-ciphers |
| aes-gcm | 0.10.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/AEADs |
| aho-corasick | 1.1.5 | Unlicense OR MIT | https://github.com/BurntSushi/aho-corasick |
| alloc-no-stdlib | 3.0.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| alloc-stdlib | 0.3.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | https://github.com/dtolnay/anyhow |
| arc-swap | 1.9.2 | MIT OR Apache-2.0 | https://github.com/vorner/arc-swap |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | https://github.com/bluss/arrayvec |
| async-compression | 0.4.50 | MIT OR Apache-2.0 | https://github.com/Nullus157/async-compression |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/atomic-waker |
| attohttpc | 0.30.1 | MPL-2.0 | https://github.com/sbstp/attohttpc |
| backon | 1.6.0 | Apache-2.0 | https://github.com/Xuanwo/backon |
| base16ct | 1.0.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| base64 | 0.21.7 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| base64 | 0.22.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| base64 | 0.23.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| base64ct | 1.8.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| bit-set | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-set |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-vec |
| bitflags | 1.3.2 | MIT/Apache-2.0 | https://github.com/bitflags/bitflags |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags |
| blake3 | 1.8.7 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | https://github.com/BLAKE3-team/BLAKE3 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| block2 | 0.6.2 | MIT | https://github.com/madsmtm/objc2 |
| brotli | 9.0.0 | BSD-3-Clause AND MIT | https://github.com/dropbox/rust-brotli |
| brotli-decompressor | 6.0.1 | BSD-3-Clause/MIT | https://github.com/dropbox/rust-brotli-decompressor |
| bs58 | 0.5.1 | MIT/Apache-2.0 | https://github.com/Nullus157/bs58-rs |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | https://github.com/fitzgen/bumpalo |
| byteorder | 1.5.0 | Unlicense OR MIT | https://github.com/BurntSushi/byteorder |
| bytes | 1.12.1 | MIT | https://github.com/tokio-rs/bytes |
| camino | 1.2.6 | MIT OR Apache-2.0 | https://github.com/camino-rs/camino |
| cargo-platform | 0.1.9 | MIT OR Apache-2.0 | https://github.com/rust-lang/cargo |
| cargo_metadata | 0.19.2 | MIT | https://github.com/oli-obk/cargo_metadata |
| cfb | 0.14.0 | MIT | https://github.com/mdsteele/rust-cfb |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/cfg-if |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/stream-ciphers |
| chrono | 0.4.45 | MIT OR Apache-2.0 | https://github.com/chronotope/chrono |
| cipher | 0.4.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| cmov | 0.5.4 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| cobs | 0.3.0 | MIT OR Apache-2.0 | https://github.com/jamesmunns/cobs.rs |
| compression-codecs | 0.4.45 | MIT OR Apache-2.0 | https://github.com/Nullus157/async-compression |
| compression-core | 0.4.33 | MIT OR Apache-2.0 | https://github.com/Nullus157/async-compression |
| const-oid | 0.10.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| constant_time_eq | 0.4.2 | CC0-1.0 OR MIT-0 OR Apache-2.0 | https://github.com/cesarb/constant_time_eq |
| convert_case | 0.10.0 | MIT | https://github.com/rutrum/convert-case |
| cookie | 0.18.2 | MIT OR Apache-2.0 | https://github.com/SergioBenitez/cookie-rs |
| cordyceps | 0.3.5 | MIT | https://github.com/hawkw/mycelium |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| core-graphics | 0.25.0 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| core-graphics-types | 0.2.0 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 | https://github.com/srijs/rust-crc32fast |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| cssparser | 0.37.0 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| cssparser-macros | 0.7.1 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| ctor | 1.0.13 | Apache-2.0 OR MIT | https://github.com/mmastrac/linktime |
| ctr | 0.9.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-modes |
| ctutils | 0.4.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| curve25519-dalek | 5.0.0 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/curve25519-dalek |
| curve25519-dalek-derive | 0.1.1 | MIT/Apache-2.0 | https://github.com/dalek-cryptography/curve25519-dalek |
| darling | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| darling_core | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| darling_macro | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| data-encoding | 2.11.1 | MIT | https://github.com/ia0/data-encoding |
| data-encoding-macro | 0.1.21 | MIT | https://github.com/ia0/data-encoding |
| data-encoding-macro-internal | 0.1.19 | MIT | https://github.com/ia0/data-encoding |
| defmt | 1.1.1 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| defmt-macros | 1.1.1 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| defmt-parser | 1.0.0 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| der | 0.8.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| deranged | 0.5.8 | MIT OR Apache-2.0 | https://github.com/jhpratt/deranged |
| derive_more | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| derive_more-impl | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| diatomic-waker | 0.2.3 | MIT OR Apache-2.0 | https://github.com/asynchronics/diatomic-waker |
| digest | 0.10.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| digest | 0.11.3 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| dirs | 7.0.0 | MIT OR Apache-2.0 | https://codeberg.org/dirs/dirs-rs |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 | https://github.com/dirs-dev/dirs-sys-rs |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | https://github.com/yaahc/displaydoc |
| dom_query | 0.28.0 | MIT | https://github.com/niklak/dom_query |
| dpi | 0.1.2 | Apache-2.0 AND MIT | https://github.com/rust-windowing/winit |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | https://github.com/dtolnay/dtoa |
| dtoa-short | 0.3.5 | MPL-2.0 | https://github.com/upsuper/dtoa-short |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | https://gitlab.com/kornelski/dunce |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 | https://github.com/dtolnay/dyn-clone |
| ed25519 | 3.0.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/signatures |
| ed25519-dalek | 3.0.0 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/ed25519-dalek |
| embed_plist | 1.2.2 | MIT OR Apache-2.0 | https://github.com/nvzqz/embed-plist-rs |
| embedded-io | 0.4.0 | MIT OR Apache-2.0 | https://github.com/embassy-rs/embedded-io |
| embedded-io | 0.6.1 | MIT OR Apache-2.0 | https://github.com/rust-embedded/embedded-hal |
| enum-assoc | 1.4.1 | MIT OR Apache-2.0 | https://github.com/Eolu/enum-assoc |
| equivalent | 1.0.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/equivalent |
| erased-serde | 0.4.10 | MIT OR Apache-2.0 | https://github.com/dtolnay/erased-serde |
| errno | 0.3.14 | MIT OR Apache-2.0 | https://github.com/lambda-fairy/rust-errno |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/fastrand |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | https://github.com/image-rs/fdeflate |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/flate2-rs |
| fnv | 1.0.7 | Apache-2.0 / MIT | https://github.com/servo/rust-fnv |
| foldhash | 0.2.0 | Zlib | https://github.com/orlp/foldhash |
| foreign-types | 0.5.0 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| foreign-types-macros | 0.2.4 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| foreign-types-shared | 0.3.1 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| futures | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-buffered | 0.2.13 | MIT | https://github.com/conradludgate/futures-buffered |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-executor | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-lite | 2.6.1 | Apache-2.0 OR MIT | https://github.com/smol-rs/futures-lite |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| generic-array | 0.14.7 | MIT | https://github.com/fizyk20/generic-array.git |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| ghash | 0.5.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| glob | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-lang/glob |
| h2 | 0.4.20 | MIT | https://github.com/hyperium/h2 |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| heck | 0.5.0 | MIT OR Apache-2.0 | https://github.com/withoutboats/heck |
| hex | 0.4.3 | MIT OR Apache-2.0 | https://github.com/KokaKiwi/rust-hex |
| html5ever | 0.39.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| http | 1.5.0 | MIT OR Apache-2.0 | https://github.com/hyperium/http |
| http-body | 1.1.0 | MIT | https://github.com/hyperium/http-body |
| http-body-util | 0.1.5 | MIT | https://github.com/hyperium/http-body |
| httparse | 1.10.1 | MIT OR Apache-2.0 | https://github.com/seanmonstar/httparse |
| httpdate | 1.0.3 | MIT OR Apache-2.0 | https://github.com/pyfisch/httpdate |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hybrid-array |
| hyper | 1.12.0 | MIT | https://github.com/hyperium/hyper |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/hyper-rustls |
| hyper-util | 0.1.21 | MIT | https://github.com/hyperium/hyper-util |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 | https://github.com/strawlab/iana-time-zone |
| ico | 0.5.0 | MIT | https://github.com/mdsteele/rust-ico |
| icu_collections | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_locale_core | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_provider | 2.3.1 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| ident_case | 1.0.1 | MIT/Apache-2.0 | https://github.com/TedDriggs/ident_case |
| identity-hash | 0.1.0 | Apache-2.0 OR MIT | https://github.com/offsetting/identity-hash |
| idna | 1.1.0 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | https://github.com/hsivonen/idna_adapter |
| igd-next | 0.17.1 | MIT | https://github.com/dariusc93/rust-igd |
| indexmap | 1.9.3 | Apache-2.0 OR MIT | https://github.com/bluss/indexmap |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap |
| infer | 0.22.0 | MIT | https://github.com/bojand/infer |
| inout | 0.1.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| ipconfig | 0.3.4 | MIT/Apache-2.0 | https://github.com/liranringel/ipconfig |
| ipnet | 2.12.2 | MIT OR Apache-2.0 | https://github.com/krisprice/ipnet |
| iroh | 1.3.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/iroh |
| iroh-base | 1.3.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/iroh |
| iroh-dns | 1.3.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/iroh |
| iroh-metrics | 1.0.2 | MIT OR Apache-2.0 | https://github.com/n0-computer/iroh-metrics |
| iroh-metrics-derive | 1.0.1 | MIT OR Apache-2.0 | https://github.com/n0-computer/iroh-metrics |
| iroh-relay | 1.3.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/iroh |
| itoa | 1.0.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/itoa |
| jiff | 0.2.38 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-core | 0.1.1 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-tzdb | 0.1.9 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-tzdb-platform | 0.1.3 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| json-patch | 4.2.0 | MIT/Apache-2.0 | https://github.com/idubrov/json-patch |
| jsonptr | 0.7.1 | MIT OR Apache-2.0 | https://github.com/chanced/jsonptr |
| keyboard-types | 0.8.3 | MIT OR Apache-2.0 | https://github.com/rust-windowing/keyboard-types |
| libc | 0.2.190 | MIT OR Apache-2.0 | https://github.com/rust-lang/libc |
| litemap | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| log | 0.4.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/log |
| lru | 0.18.5 | MIT | https://github.com/jeromefroe/lru-rs.git |
| lru-slab | 0.1.3 | MIT OR Apache-2.0 OR Zlib | https://github.com/Ralith/lru-slab |
| mac-addr | 0.3.0 | MIT | https://github.com/shellrow/mac-addr |
| markup5ever | 0.39.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| md-5 | 0.10.6 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| memchr | 2.8.3 | Unlicense OR MIT | https://github.com/BurntSushi/memchr |
| mime | 0.3.17 | MIT OR Apache-2.0 | https://github.com/hyperium/mime |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| mio | 1.2.4 | MIT | https://github.com/tokio-rs/mio |
| muda | 0.20.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/muda |
| n0-dns-resolver | 0.1.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/iroh |
| n0-error | 1.0.1 | MIT OR Apache-2.0 | https://github.com/n0-computer/n0-error |
| n0-error-macros | 1.0.1 | MIT OR Apache-2.0 | https://github.com/n0-computer/n0-error |
| n0-future | 0.3.2 | MIT OR Apache-2.0 | https://github.com/n0-computer/n0-future |
| n0-watcher | 1.0.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/n0-watcher |
| netdev | 0.45.1 | MIT | https://github.com/shellrow/netdev |
| netdev | 0.46.3 | MIT | https://github.com/shellrow/netdev |
| netwatch | 0.19.3 | MIT OR Apache-2.0 | https://github.com/n0-computer/net-tools |
| new_debug_unreachable | 1.0.6 | MIT | https://github.com/mbrubeck/rust-debug-unreachable |
| noq | 1.3.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/noq |
| noq-proto | 1.3.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/noq |
| noq-udp | 1.3.0 | MIT OR Apache-2.0 | https://github.com/n0-computer/noq |
| num-conv | 0.2.2 | MIT OR Apache-2.0 | https://github.com/jhpratt/num-conv |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | https://github.com/rust-num/num-traits |
| num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | https://github.com/illicitonion/num_enum |
| num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | https://github.com/illicitonion/num_enum |
| objc2 | 0.6.5 | MIT | https://github.com/madsmtm/objc2 |
| objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-core-graphics | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-encode | 4.1.0 | MIT | https://github.com/madsmtm/objc2 |
| objc2-exception-helper | 0.1.1 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-foundation | 0.3.2 | MIT | https://github.com/madsmtm/objc2 |
| objc2-io-surface | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-quartz-core | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-security | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-system-configuration | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-user-notifications | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-web-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | https://github.com/matklad/once_cell |
| opaque-debug | 0.3.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| option-ext | 0.2.0 | MPL-2.0 | https://github.com/soc/option-ext.git |
| papaya | 0.2.5 | MIT | https://github.com/ibraheemdev/papaya |
| parking | 2.2.1 | Apache-2.0 OR MIT | https://github.com/smol-rs/parking |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| pem-rfc7468 | 1.0.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| phf | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_generator | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_macros | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_shared | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| pin-project | 1.1.13 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project |
| pin-project-internal | 1.1.13 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project-lite |
| pkcs8 | 0.11.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| plist | 1.10.1 | MIT | https://github.com/ebarnard/rust-plist/ |
| png | 0.17.16 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| png | 0.18.1 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| polyval | 0.6.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT | https://github.com/taiki-e/portable-atomic |
| portmapper | 0.19.3 | MIT OR Apache-2.0 | https://github.com/n0-computer/net-tools |
| postcard | 1.1.3 | MIT OR Apache-2.0 | https://github.com/jamesmunns/postcard |
| postcard-derive | 0.2.2 | MIT OR Apache-2.0 | https://github.com/jamesmunns/postcard |
| potential_utf | 0.1.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 | https://github.com/jhpratt/powerfmt |
| precomputed-hash | 0.1.1 | MIT | https://github.com/emilio/precomputed-hash |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | https://github.com/bkchr/proc-macro-crate |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 |
| quick-xml | 0.42.0 | MIT | https://github.com/tafia/quick-xml |
| quinn | 0.11.12 | MIT OR Apache-2.0 | https://github.com/quinn-rs/quinn |
| quinn-proto | 0.11.19 | MIT OR Apache-2.0 | https://github.com/quinn-rs/quinn |
| quinn-udp | 0.5.16 | MIT OR Apache-2.0 | https://github.com/quinn-rs/quinn |
| quote | 1.0.47 | MIT OR Apache-2.0 | https://github.com/dtolnay/quote |
| rand | 0.10.3 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand_core | 0.10.1 | MIT OR Apache-2.0 | https://github.com/rust-random/rand_core |
| rand_pcg | 0.10.2 | MIT OR Apache-2.0 | https://github.com/rust-random/rngs |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | https://github.com/rust-windowing/raw-window-handle |
| ref-cast | 1.0.27 | MIT OR Apache-2.0 | https://github.com/dtolnay/ref-cast |
| ref-cast-impl | 1.0.27 | MIT OR Apache-2.0 | https://github.com/dtolnay/ref-cast |
| regex | 1.13.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| reqwest | 0.12.28 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| reqwest | 0.13.5 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| rfd | 0.16.0 | MIT | https://github.com/PolyMeilex/rfd |
| ring | 0.17.14 | Apache-2.0 AND ISC | https://github.com/briansmith/ring |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | https://github.com/rust-lang/rustc-hash |
| rustls | 0.23.45 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/rustls |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | https://github.com/rustls/pki-types |
| rustls-platform-verifier | 0.7.1 | MIT OR Apache-2.0 | https://github.com/rustls/rustls-platform-verifier |
| rustls-webpki | 0.103.15 | ISC | https://github.com/rustls/webpki |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | https://github.com/dtolnay/rustversion |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | https://github.com/dtolnay/ryu |
| same-file | 1.0.6 | Unlicense/MIT | https://github.com/BurntSushi/same-file |
| schemars | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| schemars | 0.9.0 | MIT | https://github.com/GREsau/schemars |
| schemars | 1.2.2 | MIT | https://github.com/GREsau/schemars |
| schemars_derive | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | https://github.com/bluss/scopeguard |
| security-framework | 3.7.0 | MIT OR Apache-2.0 | https://github.com/kornelski/rust-security-framework |
| security-framework-sys | 2.17.0 | MIT OR Apache-2.0 | https://github.com/kornelski/rust-security-framework |
| seize | 0.5.1 | MIT | https://github.com/ibraheemdev/seize |
| selectors | 0.38.0 | MPL-2.0 | https://github.com/servo/stylo |
| semver | 1.0.28 | MIT OR Apache-2.0 | https://github.com/dtolnay/semver |
| serde | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde-untagged | 0.1.9 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-untagged |
| serde_bytes | 0.11.19 | MIT OR Apache-2.0 | https://github.com/serde-rs/bytes |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-repr |
| serde_spanned | 0.6.9 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 | https://github.com/nox/serde_urlencoded |
| serde_with | 3.24.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| serde_with_macros | 3.24.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| serdect | 0.4.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| servo_arc | 0.4.3 | MIT OR Apache-2.0 | https://github.com/servo/stylo |
| sha1 | 0.10.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| sha1 | 0.11.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| sha1_smol | 1.0.1 | BSD-3-Clause | https://github.com/mitsuhiko/sha1-smol |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| sha2 | 0.11.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 | https://github.com/vorner/signal-hook |
| signature | 3.0.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/traits |
| simd-adler32 | 0.3.10 | MIT | https://github.com/mcountryman/simd-adler32 |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | https://github.com/rusticstuff/simdutf8 |
| simple-dns | 0.12.0 | MIT | https://github.com/balliegojr/simple-dns |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | https://github.com/jedisct1/rust-siphash |
| slab | 0.4.12 | MIT | https://github.com/tokio-rs/slab |
| smallvec | 1.16.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-smallvec |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/socket2 |
| softbuffer | 0.4.8 | MIT OR Apache-2.0 | https://github.com/rust-windowing/softbuffer |
| sorted-index-buffer | 0.2.1 | MIT OR Apache-2.0 | https://github.com/n0-computer/iroh |
| spez | 0.1.2 | BSD-2-Clause | https://github.com/m-ou-se/spez |
| spin | 0.10.1 | MIT | https://github.com/mvdnes/spin-rs.git |
| spki | 0.8.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | https://github.com/storyyeller/stable_deref_trait |
| string_cache | 0.9.0 | MIT OR Apache-2.0 | https://github.com/servo/string-cache |
| strsim | 0.11.1 | MIT | https://github.com/rapidfuzz/strsim-rs |
| strum | 0.28.0 | MIT | https://github.com/Peternator7/strum |
| strum_macros | 0.28.0 | MIT | https://github.com/Peternator7/strum |
| subtle | 2.6.1 | BSD-3-Clause | https://github.com/dalek-cryptography/subtle |
| swift-rs | 1.0.8 | MIT OR Apache-2.0 | https://github.com/Brendonovich/swift-rs |
| syn | 2.0.119 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| syn | 3.0.6 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| sync_wrapper | 1.0.2 | Apache-2.0 | https://github.com/Actyx/sync_wrapper |
| synstructure | 0.14.0 | MIT | https://github.com/mystor/synstructure |
| system-configuration | 0.8.0 | MIT OR Apache-2.0 | https://github.com/mullvad/system-configuration-rs |
| system-configuration-sys | 0.6.0 | MIT OR Apache-2.0 | https://github.com/mullvad/system-configuration-rs |
| tao | 0.37.1 | Apache-2.0 | https://github.com/tauri-apps/tao |
| tauri | 2.12.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-codegen | 2.7.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-macros | 2.7.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-plugin-dialog | 2.8.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-fs | 2.6.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-runtime | 2.12.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-runtime-wry | 2.12.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-utils | 2.10.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tendril | 0.5.1 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| time | 0.3.55 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| time-core | 0.1.9 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| time-macros | 0.2.32 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| tinystr | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/tinyvec |
| tokio | 1.53.2 | MIT | https://github.com/tokio-rs/tokio |
| tokio-macros | 2.7.2 | MIT | https://github.com/tokio-rs/tokio |
| tokio-rustls | 0.26.6 | MIT OR Apache-2.0 | https://github.com/rustls/tokio-rustls |
| tokio-stream | 0.1.19 | MIT | https://github.com/tokio-rs/tokio |
| tokio-tungstenite | 0.30.0 | MIT | https://github.com/snapview/tokio-tungstenite |
| tokio-util | 0.7.19 | MIT | https://github.com/tokio-rs/tokio |
| tokio-websockets | 0.13.3 | MIT | https://github.com/Gelbpunkt/tokio-websockets/ |
| toml | 0.8.2 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml | 1.1.6+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_datetime | 0.6.3 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_datetime | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_edit | 0.20.2 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_edit | 0.25.15+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_parser | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_writer | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| tower | 0.5.3 | MIT | https://github.com/tower-rs/tower |
| tower-http | 0.6.11 | MIT | https://github.com/tower-rs/tower-http |
| tower-layer | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| tower-service | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| tracing | 0.1.44 | MIT | https://github.com/tokio-rs/tracing |
| tracing-attributes | 0.1.31 | MIT | https://github.com/tokio-rs/tracing |
| tracing-core | 0.1.36 | MIT | https://github.com/tokio-rs/tracing |
| trash | 5.2.9 | MIT | https://github.com/ArturKovacs/trash |
| tray-icon | 0.25.1 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tray-icon |
| try-lock | 0.2.5 | MIT | https://github.com/seanmonstar/try-lock |
| tungstenite | 0.30.0 | MIT OR Apache-2.0 | https://github.com/snapview/tungstenite-rs |
| typeid | 1.0.3 | MIT OR Apache-2.0 | https://github.com/dtolnay/typeid |
| typenum | 1.20.1 | MIT OR Apache-2.0 | https://github.com/paholg/typenum |
| unicode-ident | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | https://github.com/dtolnay/unicode-ident |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-segmentation |
| unicode-xid | 0.2.6 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-xid |
| universal-hash | 0.5.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| untrusted | 0.9.0 | ISC | https://github.com/briansmith/untrusted |
| url | 2.5.8 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| urlpattern | 0.6.0 | MIT | https://github.com/denoland/rust-urlpattern |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | https://github.com/hsivonen/utf8_iter |
| uuid | 1.27.0 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| walkdir | 2.5.0 | Unlicense/MIT | https://github.com/BurntSushi/walkdir |
| want | 0.3.2 | MIT | https://github.com/seanmonstar/want |
| web-time | 1.1.0 | MIT OR Apache-2.0 | https://github.com/daxpedda/web-time |
| web_atoms | 0.2.6 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| webpki-roots | 0.26.11 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots |
| webpki-roots | 1.0.9 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots |
| webview2-com | 0.39.1 | MIT | https://github.com/wravery/webview2-rs |
| webview2-com-macros | 0.8.1 | MIT | https://github.com/wravery/webview2-rs |
| webview2-com-sys | 0.39.1 | MIT | https://github.com/wravery/webview2-rs |
| widestring | 1.2.1 | MIT OR Apache-2.0 | https://github.com/VoidStarKat/widestring-rs |
| winapi-util | 0.1.11 | Unlicense OR MIT | https://github.com/BurntSushi/winapi-util |
| window-vibrancy | 0.8.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri-plugin-vibrancy |
| windows | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-core | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-future | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-registry | 0.6.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result | 0.4.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-version | 0.1.7 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| winnow | 0.5.40 | MIT | https://github.com/winnow-rs/winnow |
| winnow | 1.0.4 | MIT | https://github.com/winnow-rs/winnow |
| wmi | 0.18.4 | MIT OR Apache-2.0 | https://github.com/ohadravid/wmi-rs |
| writeable | 0.6.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| wry | 0.57.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/wry |
| xml-rs | 0.8.29 | MIT | https://github.com/kornelski/xml-rs |
| xmltree | 0.10.3 | MIT | https://github.com/eminence/xmltree-rs |
| yoke | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| yoke-derive | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerofrom | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| zeroize_derive | 1.5.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| zerotrie | 0.2.5 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec | 0.11.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec-derive | 0.11.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zip | 2.4.2 | MIT | https://github.com/zip-rs/zip2.git |
| zlib-rs | 0.6.8 | Zlib | https://github.com/trifectatechfoundation/zlib-rs |
| zmij | 1.0.23 | MIT | https://github.com/dtolnay/zmij |
| zopfli | 0.8.3 | Apache-2.0 | https://github.com/zopfli-rs/zopfli |

## 3. Avisos de copyright

Las licencias MIT, BSD, ISC y similares obligan a conservar el aviso de copyright junto al texto de la licencia. Aquí están los avisos que aparecen en los ficheros de licencia de cada paquete, por nombre.

- Inter (fuente): Copyright 2016 The Inter Project Authors (https://github.com/rsms/inter)
- adler2 2.0.1: Copyright (C) Jonas Schievink <jonasschievink@gmail.com>
- aead 0.5.2: Copyright (c) 2019 The RustCrypto Project Developers
- aead 0.5.2: Copyright (c) 2019 MobileCoin, LLC
- aes 0.8.4: Copyright (c) 2018 Artyom Pavlov
- aes-gcm 0.10.3: Copyright (c) 2019 The RustCrypto Project Developers
- aho-corasick 1.1.5: Copyright (c) 2015 Andrew Gallant
- alloc-no-stdlib 3.0.0: Copyright (c) 2016 Dropbox, Inc.
- arc-swap 1.9.2: Copyright (c) 2017 arc-swap developers
- arrayvec 0.7.8: Copyright (c) Ulrik Sverdrup "bluss" 2015-2023
- async-compression 0.4.50: Copyright (c) 2018 the rustasync developers
- atomic-waker 1.1.2: Copyright (c) 2016 Alex Crichton
- atomic-waker 1.1.2: Copyright (c) 2017 The Tokio Authors
- base16ct 1.0.0: Copyright (c) 2014 Steve "Sc00bz" Thomas (steve at tobtu dot com)
- base16ct 1.0.0: Copyright (c) 2022-2025 The RustCrypto Project Developers
- base64 0.21.7: Copyright (c) 2015 Alice Maz
- base64 0.22.1: Copyright (c) 2015 Alice Maz
- base64 0.23.1: Copyright (c) 2025 Alice Maz, Marshall Pierce
- base64ct 1.8.3: Copyright (c) 2014 Steve "Sc00bz" Thomas (steve at tobtu dot com)
- base64ct 1.8.3: Copyright (c) 2021-2025 The RustCrypto Project Developers
- bit-set 0.8.0: Copyright (c) 2023 The Rust Project Developers
- bit-vec 0.8.0: Copyright (c) 2023 The Rust Project Developers
- bitflags 1.3.2: Copyright (c) 2014 The Rust Project Developers
- bitflags 2.13.2: Copyright (c) 2014 The Rust Project Developers
- block-buffer 0.10.4: Copyright (c) 2018-2019 The RustCrypto Project Developers
- block-buffer 0.12.1: Copyright (c) 2018-2025 The RustCrypto Project Developers
- brotli 9.0.0: Copyright (c) 2016 Dropbox, Inc.
- brotli 9.0.0: Copyright (c) 2009, 2010, 2013-2016 by the Brotli Authors.
- brotli-decompressor 6.0.1: Copyright (c) 2016 Dropbox, Inc.
- bs58 0.5.1: Copyright (c) 2016 The roaring-rs developers.
- bumpalo 3.20.3: Copyright (c) 2019 Nick Fitzgerald
- byteorder 1.5.0: Copyright (c) 2015 Andrew Gallant
- bytes 1.12.1: Copyright (c) 2018 Carl Lerche
- cfb 0.14.0: Copyright (c) 2017 Matthew D. Steele
- cfg-if 1.0.5: Copyright (c) 2014 Alex Crichton
- chacha20 0.10.2: Copyright (c) 2019-2026 The RustCrypto Project Developers
- cipher 0.4.4: Copyright (c) 2016-2020 RustCrypto Developers
- cmov 0.5.4: Copyright (c) 2022-2026 The RustCrypto Project Developers
- cobs 0.3.0: Copyright (c) 2015 The cobs.rs Developers
- compression-codecs 0.4.45: Copyright (c) 2018 the rustasync developers
- compression-core 0.4.33: Copyright (c) 2018 the rustasync developers
- const-oid 0.10.2: Copyright (c) 2020-2026 The RustCrypto Project Developers
- convert_case 0.10.0: Copyright (c) 2025 rutrum
- cookie 0.18.2: Copyright (c) 2017 Sergio Benitez
- cookie 0.18.2: Copyright (c) 2014 Alex Crichton
- cordyceps 0.3.5: Copyright (c) 2022 Eliza Weisman
- core-foundation 0.10.1: Copyright (c) 2012-2013 Mozilla Foundation
- core-foundation-sys 0.8.7: Copyright (c) 2012-2013 Mozilla Foundation
- core-graphics 0.25.0: Copyright (c) 2012-2013 Mozilla Foundation
- core-graphics-types 0.2.0: Copyright (c) 2012-2013 Mozilla Foundation
- cpufeatures 0.2.17: Copyright (c) 2020-2025 The RustCrypto Project Developers
- cpufeatures 0.3.1: Copyright (c) 2020-2026 The RustCrypto Project Developers
- crc32fast 1.5.2: Copyright (c) 2018 Sam Rijs, Alex Crichton and contributors
- crossbeam-channel 0.5.17: Copyright (c) 2019 The Crossbeam Project Developers
- crossbeam-utils 0.8.23: Copyright (c) 2019 The Crossbeam Project Developers
- crypto-common 0.1.7: Copyright (c) 2021 RustCrypto Developers
- crypto-common 0.2.2: Copyright (c) 2021-2026 RustCrypto Developers
- ctr 0.9.2: Copyright (c) 2018-2022 RustCrypto Developers
- ctr 0.9.2: Copyright (c) 2018 Artyom Pavlov
- ctutils 0.4.3: Copyright (c) 2025-2026 The RustCrypto Project Developers
- curve25519-dalek 5.0.0: Copyright (c) 2016-2021 isis agora lovecruft. All rights reserved.
- curve25519-dalek 5.0.0: Copyright (c) 2016-2021 Henry de Valence. All rights reserved.
- darling 0.24.1: Copyright (c) 2017 Ted Driggs
- darling_core 0.24.1: Copyright (c) 2017 Ted Driggs
- darling_macro 0.24.1: Copyright (c) 2017 Ted Driggs
- data-encoding 2.11.1: Copyright (c) 2015-2020 Julien Cretin
- data-encoding 2.11.1: Copyright (c) 2017-2020 Google Inc.
- data-encoding-macro 0.1.21: Copyright (c) 2015-2020 Julien Cretin
- data-encoding-macro 0.1.21: Copyright (c) 2017-2020 Google Inc.
- data-encoding-macro-internal 0.1.19: Copyright (c) 2015-2020 Julien Cretin
- data-encoding-macro-internal 0.1.19: Copyright (c) 2017-2020 Google Inc.
- defmt 1.1.1: Copyright (c) Ferrous Systems
- defmt-macros 1.1.1: Copyright (c) Ferrous Systems
- der 0.8.2: Copyright (c) 2020-2026 The RustCrypto Project Developers
- deranged 0.5.8: Copyright (c) 2024 Jacob Pratt et al.
- derive_more 2.1.1: Copyright (c) 2016 Jelte Fennema
- derive_more-impl 2.1.1: Copyright (c) 2016 Jelte Fennema
- diatomic-waker 0.2.3: Copyright (c) 2024 Asynchronics and contributors
- digest 0.10.7: Copyright (c) 2017 Artyom Pavlov
- digest 0.11.3: Copyright (c) 2017-2025 RustCrypto Developers
- digest 0.11.3: Copyright (c) 2017 Artyom Pavlov
- dirs 7.0.0: Copyright (c) 2018-2019 dirs-rs contributors
- dirs-sys 0.5.0: Copyright (c) 2018-2019 dirs-rs contributors
- dom_query 0.28.0: Copyright (c) 2023 Mykola Humanov
- dpi 0.1.2: Copyright (c) 2018 Jorge Aparicio
- dpi 0.1.2: Copyright © 2005-2020 Rich Felker, et al.
- dpi 0.1.2: Copyright © 1993,2004 Sun Microsystems or
- dpi 0.1.2: Copyright © 2003-2011 David Schultz or
- dpi 0.1.2: Copyright © 2003-2009 Steven G. Kargl or
- dpi 0.1.2: Copyright © 2003-2009 Bruce D. Evans or
- dpi 0.1.2: Copyright © 2008 Stephen L. Moshier or
- dpi 0.1.2: Copyright © 2017-2018 Arm Limited
- ed25519 3.0.0: Copyright (c) 2018-2026 RustCrypto Developers
- ed25519-dalek 3.0.0: Copyright (c) 2017-2019 isis agora lovecruft. All rights reserved.
- embed_plist 1.2.2: Copyright (c) 2020 Nikolai Vazquez
- embedded-io 0.4.0: Copyright (c) 2022 The embedded-io authors
- embedded-io 0.6.1: Copyright (c) 2023 The embedded-io authors
- equivalent 1.0.2: Copyright (c) 2016--2023
- errno 0.3.14: Copyright (c) 2014 Chris Wong
- flate2 1.1.10: Copyright (c) 2014-2026 Alex Crichton
- fnv 1.0.7: Copyright (c) 2017 Contributors
- foldhash 0.2.0: Copyright (c) 2024 Orson Peters
- foreign-types 0.5.0: Copyright (c) 2017 The foreign-types Developers
- foreign-types-macros 0.2.4: Copyright (c) 2017 The foreign-types Developers
- foreign-types-shared 0.3.1: Copyright (c) 2017 The foreign-types Developers
- form_urlencoded 1.2.2: Copyright (c) 2013-2016 The rust-url developers
- futures 0.3.34: Copyright (c) 2016 Alex Crichton
- futures 0.3.34: Copyright (c) 2017 The Tokio Authors
- futures-buffered 0.2.13: Copyright (c) 2022 Conrad Ludgate
- futures-channel 0.3.34: Copyright (c) 2016 Alex Crichton
- futures-channel 0.3.34: Copyright (c) 2017 The Tokio Authors
- futures-core 0.3.34: Copyright (c) 2016 Alex Crichton
- futures-core 0.3.34: Copyright (c) 2017 The Tokio Authors
- futures-executor 0.3.34: Copyright (c) 2016 Alex Crichton
- futures-executor 0.3.34: Copyright (c) 2017 The Tokio Authors
- futures-io 0.3.34: Copyright (c) 2016 Alex Crichton
- futures-io 0.3.34: Copyright (c) 2017 The Tokio Authors
- futures-lite 2.6.1: Copyright (c) 2016 Alex Crichton
- futures-lite 2.6.1: Copyright (c) 2017 The Tokio Authors
- futures-macro 0.3.34: Copyright (c) 2016 Alex Crichton
- futures-macro 0.3.34: Copyright (c) 2017 The Tokio Authors
- futures-sink 0.3.34: Copyright (c) 2016 Alex Crichton
- futures-sink 0.3.34: Copyright (c) 2017 The Tokio Authors
- futures-task 0.3.34: Copyright (c) 2016 Alex Crichton
- futures-task 0.3.34: Copyright (c) 2017 The Tokio Authors
- futures-util 0.3.34: Copyright (c) 2016 Alex Crichton
- futures-util 0.3.34: Copyright (c) 2017 The Tokio Authors
- generic-array 0.14.7: Copyright (c) 2015 Bartłomiej Kamiński
- getrandom 0.2.17: Copyright (c) 2018-2024 The rust-random Project Developers
- getrandom 0.2.17: Copyright (c) 2014 The Rust Project Developers
- getrandom 0.3.4: Copyright (c) 2018-2025 The rust-random Project Developers
- getrandom 0.3.4: Copyright (c) 2014 The Rust Project Developers
- getrandom 0.4.3: Copyright (c) 2018-2026 The rust-random Project Developers
- getrandom 0.4.3: Copyright (c) 2014 The Rust Project Developers
- ghash 0.5.1: Copyright (c) 2019 RustCrypto Developers
- glob 0.3.4: Copyright (c) 2014 The Rust Project Developers
- h2 0.4.20: Copyright (c) 2017 h2 authors
- hashbrown 0.12.3: Copyright (c) 2016 Amanieu d'Antras
- hashbrown 0.17.1: Copyright (c) 2016 Amanieu d'Antras
- heck 0.5.0: Copyright (c) 2015 The Rust Project Developers
- hex 0.4.3: Copyright (c) 2013-2014 The Rust Project Developers.
- hex 0.4.3: Copyright (c) 2015-2020 The rust-hex Developers
- html5ever 0.39.0: Copyright (c) 2014 The html5ever Project Developers
- http 1.5.0: Copyright (c) 2017 http-rs authors
- http-body 1.1.0: Copyright (c) 2019-2026 Sean McArthur & Hyper Contributors
- http-body-util 0.1.5: Copyright (c) 2019-2026 Sean McArthur & Hyper Contributors
- httparse 1.10.1: Copyright (c) 2015-2025 Sean McArthur
- httpdate 1.0.3: Copyright (c) 2016 Pyfisch
- hybrid-array 0.4.15: Copyright (c) 2022-2026 The RustCrypto Project Developers
- hyper 1.12.0: Copyright (c) 2014-2026 Sean McArthur
- hyper-rustls 0.27.10: Copyright (c) 2016, Joseph Birr-Pixton <jpixton@gmail.com>
- hyper-rustls 0.27.10: Copyright (c) 2016 Joseph Birr-Pixton <jpixton@gmail.com>
- hyper-util 0.1.21: Copyright (c) 2023-2025 Sean McArthur
- iana-time-zone 0.1.65: Copyright (c) 2020 Andrew D. Straw
- ico 0.5.0: Copyright (c) 2018 Matthew D. Steele
- icu_collections 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_locale_core 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_normalizer 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_normalizer_data 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_properties 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_properties_data 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_provider 2.3.1: Copyright © 2020-2024 Unicode, Inc.
- identity-hash 0.1.0: Copyright 2018 Parity Technologies (UK) Ltd.
- identity-hash 0.1.0: Copyright 2023 Team Offsetting
- idna 1.1.0: Copyright (c) 2013-2025 The rust-url developers
- idna_adapter 1.2.2: Copyright (c) The rust-url developers
- igd-next 0.17.1: Copyright (c) 2015 Simon Bernier St-Pierre
- indexmap 1.9.3: Copyright (c) 2016--2017
- indexmap 2.14.2: Copyright (c) 2016--2017
- infer 0.22.0: Copyright (c) 2019 Bojan
- inout 0.1.4: Copyright (c) 2022 The RustCrypto Project Developers
- inout 0.1.4: Copyright (c) 2022 Artyom Pavlov
- ipconfig 0.3.4: Copyright (c) 2017 Liran Ringel
- ipnet 2.12.2: Copyright 2017 Juniper Networks, Inc.
- iroh 1.3.0: Copyright (c) 2020 Tailscale Inc & AUTHORS.
- iroh-metrics 1.0.2: Copyright 2026 N0, INC.
- iroh-relay 1.3.0: Copyright (c) 2020 Tailscale Inc & AUTHORS.
- jiff 0.2.38: Copyright (c) 2015 Andrew Gallant
- jiff-core 0.1.1: Copyright (c) 2015 Andrew Gallant
- jiff-tzdb 0.1.9: Copyright (c) 2015 Andrew Gallant
- jiff-tzdb-platform 0.1.3: Copyright (c) 2015 Andrew Gallant
- json-patch 4.2.0: Copyright (c) 2017 Ivan Dubrov
- jsonptr 0.7.1: Copyright (c) 2022 Chance Dinkins
- keyboard-types 0.8.3: Copyright (c) 2017 Pyfisch
- libc 0.2.190: Copyright (c) The Rust Project Developers
- litemap 0.8.3: Copyright © 2020-2024 Unicode, Inc.
- lock_api 0.4.14: Copyright (c) 2016 The Rust Project Developers
- log 0.4.34: Copyright (c) 2014 The Rust Project Developers
- lru 0.18.5: Copyright (c) 2016 Jerome Froelich
- lru-slab 0.1.3: Copyright (c) 2024 The lru-slab Developers
- mac-addr 0.3.0: Copyright (c) 2025 shellrow
- markup5ever 0.39.0: Copyright (c) 2014 The html5ever Project Developers
- md-5 0.10.6: Copyright (c) 2006-2009 Graydon Hoare
- md-5 0.10.6: Copyright (c) 2009-2013 Mozilla Foundation
- md-5 0.10.6: Copyright (c) 2016 Artyom Pavlov
- memchr 2.8.3: Copyright (c) 2015 Andrew Gallant
- mime 0.3.17: Copyright (c) 2014 Sean McArthur
- miniz_oxide 0.8.9: Copyright 2013-2014 RAD Game Tools and Valve Software
- miniz_oxide 0.8.9: Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC
- miniz_oxide 0.8.9: Copyright (c) 2017 Frommi
- miniz_oxide 0.8.9: Copyright (c) 2017-2024 oyvindln
- miniz_oxide 0.8.9: Copyright (c) 2020 Frommi
- miniz_oxide 0.9.1: Copyright 2013-2014 RAD Game Tools and Valve Software
- miniz_oxide 0.9.1: Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC
- miniz_oxide 0.9.1: Copyright (c) 2017 Frommi
- miniz_oxide 0.9.1: Copyright (c) 2017-2024 oyvindln
- miniz_oxide 0.9.1: Copyright (c) 2020 Frommi
- mio 1.2.4: Copyright (c) 2014 Carl Lerche and other MIO contributors
- muda 0.20.0: Copyright (c) 2022-2022 Tauri Programme within The Commons Conservancy
- n0-dns-resolver 0.1.0: Copyright 2025 N0, INC.
- n0-error 1.0.1: Copyright 2026 N0, INC.
- n0-future 0.3.2: Copyright 2023 N0, INC.
- n0-watcher 1.0.0: Copyright 2026 N0, INC.
- netdev 0.45.1: Copyright (c) 2021-2026 shellrow
- netdev 0.46.3: Copyright (c) 2021-2026 shellrow
- new_debug_unreachable 1.0.6: Copyright (c) 2015 Jonathan Reem
- noq 1.3.0: Copyright (c) 2018-2025 The quinn Developers
- noq 1.3.0: Copyright (c) 2025 N0, Inc.
- noq-proto 1.3.0: Copyright (c) 2018-2025 The quinn Developers
- noq-proto 1.3.0: Copyright (c) 2025 N0, Inc.
- noq-udp 1.3.0: Copyright (c) 2018-2025 The quinn Developers
- noq-udp 1.3.0: Copyright (c) 2025 N0, Inc.
- num-conv 0.2.2: Copyright (c) Jacob Pratt
- num-traits 0.2.19: Copyright (c) 2014 The Rust Project Developers
- num_enum 0.7.6: Copyright (c) 2018, Daniel Wagner-Hall
- num_enum_derive 0.7.6: Copyright (c) 2018, Daniel Wagner-Hall
- opaque-debug 0.3.1: Copyright (c) 2018-2024 The RustCrypto Project Developers
- parking 2.2.1: Copyright 2014-2020 The Rust Project Developers
- parking_lot 0.12.5: Copyright (c) 2016 The Rust Project Developers
- parking_lot_core 0.9.12: Copyright (c) 2016 The Rust Project Developers
- pem-rfc7468 1.0.0: Copyright (c) 2021-2025 The RustCrypto Project Developers
- percent-encoding 2.3.2: Copyright (c) 2013-2025 The rust-url developers
- phf 0.13.1: Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi
- phf_generator 0.13.1: Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi
- phf_macros 0.13.1: Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi
- phf_shared 0.13.1: Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi
- pkcs8 0.11.0: Copyright (c) 2020-2026 The RustCrypto Project Developers
- plist 1.10.1: Copyright (c) 2015 Edward Barnard
- png 0.17.16: Copyright (c) 2015 nwin
- png 0.18.1: Copyright (c) 2015 nwin
- polyval 0.6.2: Copyright (c) 2019-2023 RustCrypto Developers
- postcard 1.1.3: Copyright (c) 2019 Anthony James Munns
- postcard-derive 0.2.2: Copyright (c) 2019 Anthony James Munns
- potential_utf 0.1.6: Copyright © 2020-2024 Unicode, Inc.
- powerfmt 0.2.1: Copyright (c) 2023 Jacob Pratt et al.
- precomputed-hash 0.1.1: Copyright (c) 2017 Emilio Cobos Álvarez
- quick-xml 0.42.0: Copyright (c) 2016 Johann Tuffe
- quinn 0.11.12: Copyright (c) 2018 The quinn Developers
- quinn-proto 0.11.19: Copyright (c) 2018 The quinn Developers
- quinn-udp 0.5.16: Copyright (c) 2018 The quinn Developers
- rand 0.10.3: Copyright 2018 Developers of the Rand project
- rand 0.10.3: Copyright (c) 2014 The Rust Project Developers
- rand_core 0.10.1: Copyright (c) 2018-2026 The Rand Project Developers
- rand_pcg 0.10.2: Copyright (c) 2014-2017 Melissa O'Neill and PCG Project contributors
- rand_pcg 0.10.2: Copyright 2018 Developers of the Rand project
- raw-window-handle 0.6.2: Copyright (c) 2019 Osspial
- raw-window-handle 0.6.2: Copyright (c) 2020 Osspial
- regex 1.13.1: Copyright (c) 2014 The Rust Project Developers
- regex-automata 0.4.18: Copyright (c) 2014 The Rust Project Developers
- regex-syntax 0.8.11: Copyright (c) 2014 The Rust Project Developers
- reqwest 0.12.28: Copyright (c) 2016-2025 Sean McArthur
- reqwest 0.13.5: Copyright (c) 2016-2026 Sean McArthur
- rfd 0.16.0: Copyright (c) 2022 Bartłomiej Maryńczak
- ring 0.17.14: Copyright 2015-2025 Brian Smith.
- rustls 0.23.45: Copyright (c) 2016, Joseph Birr-Pixton <jpixton@gmail.com>
- rustls 0.23.45: Copyright (c) 2016 Joseph Birr-Pixton <jpixton@gmail.com>
- rustls-pki-types 1.15.1: Copyright (c) 2023 Dirkjan Ochtman <dirkjan@ochtman.nl>
- rustls-platform-verifier 0.7.1: Copyright (c) 2022 1Password
- rustls-webpki 0.103.15: Copyright 2015 Brian Smith.
- same-file 1.0.6: Copyright (c) 2017 Andrew Gallant
- schemars 0.8.22: Copyright (c) 2019 Graham Esau
- schemars 0.9.0: Copyright (c) 2019 Graham Esau
- schemars 1.2.2: Copyright (c) 2019 Graham Esau
- schemars_derive 0.8.22: Copyright (c) 2019 Graham Esau
- scopeguard 1.2.0: Copyright (c) 2016-2019 Ulrik Sverdrup "bluss" and scopeguard developers
- security-framework 3.7.0: Copyright (c) 2015 Steven Fackler
- security-framework-sys 2.17.0: Copyright (c) 2015 Steven Fackler
- seize 0.5.1: Copyright (c) 2022 Ibraheem Ahmed
- serde_spanned 0.6.9: Copyright (c) Individual contributors
- serde_spanned 1.1.1: Copyright (c) Individual contributors
- serde_urlencoded 0.7.1: Copyright (c) 2016 Anthony Ramine
- serde_with 3.24.0: Copyright (c) 2015
- serde_with_macros 3.24.0: Copyright (c) 2015
- serdect 0.4.3: Copyright (c) 2022-2025 The RustCrypto Project Developers
- serialize-to-javascript 0.1.2: Copyright (c) 2021 Chip Reed
- serialize-to-javascript-impl 0.1.2: Copyright (c) 2021 Chip Reed
- sha1 0.10.7: Copyright (c) 2006-2009 Graydon Hoare
- sha1 0.10.7: Copyright (c) 2009-2013 Mozilla Foundation
- sha1 0.10.7: Copyright (c) 2016 Artyom Pavlov
- sha1 0.11.0: Copyright (c) 2016-2026 The RustCrypto Project Developers
- sha1 0.11.0: Copyright (c) 2016 Artyom Pavlov
- sha1 0.11.0: Copyright (c) 2009-2013 Mozilla Foundation
- sha1 0.11.0: Copyright (c) 2006-2009 Graydon Hoare
- sha1_smol 1.0.1: Copyright (c) 2018, the respective contributors, as shown by the AUTHORS file.
- sha1_smol 1.0.1: Copyright (c) 2006-2009 Graydon Hoare
- sha1_smol 1.0.1: Copyright (c) 2009-2013 Mozilla Foundation
- sha2 0.10.9: Copyright (c) 2006-2009 Graydon Hoare
- sha2 0.10.9: Copyright (c) 2009-2013 Mozilla Foundation
- sha2 0.10.9: Copyright (c) 2016 Artyom Pavlov
- sha2 0.11.0: Copyright (c) 2016-2026 The RustCrypto Project Developers
- sha2 0.11.0: Copyright (c) 2016 Artyom Pavlov
- sha2 0.11.0: Copyright (c) 2009-2013 Mozilla Foundation
- sha2 0.11.0: Copyright (c) 2006-2009 Graydon Hoare
- signal-hook-registry 1.4.8: Copyright (c) 2017 tokio-jsonrpc developers
- signature 3.0.0: Copyright (c) 2018-2026 RustCrypto Developers
- simd-adler32 0.3.10: Copyright (c) [2021] [Marvin Countryman]
- simple-dns 0.12.0: Copyright (c) 2021 Ilson Roberto Balliego Junior
- siphasher 1.0.4: Copyright 2012-2016 The Rust Project Developers.
- siphasher 1.0.4: Copyright 2016-2026 Frank Denis.
- slab 0.4.12: Copyright (c) 2019 Carl Lerche
- smallvec 1.16.2: Copyright (c) 2018 The Servo Project Developers
- socket2 0.6.5: Copyright (c) 2014 Alex Crichton
- softbuffer 0.4.8: Copyright 2022 Kirill Chibisov
- sorted-index-buffer 0.2.1: Copyright 2025 N0, INC.
- spez 0.1.2: Copyright (c) 2020, Mara Bos <m-ou.se@m-ou.se>
- spin 0.10.1: Copyright (c) 2014 Mathijs van de Nes
- spki 0.8.1: Copyright (c) 2021-2026 The RustCrypto Project Developers
- stable_deref_trait 1.2.1: Copyright (c) 2017 Robert Grosse
- string_cache 0.9.0: Copyright (c) 2012-2013 Mozilla Foundation
- strsim 0.11.1: Copyright (c) 2015 Danny Guo
- strsim 0.11.1: Copyright (c) 2016 Titus Wormer <tituswormer@gmail.com>
- strsim 0.11.1: Copyright (c) 2018 Akash Kurdekar
- strum 0.28.0: Copyright (c) 2019 Peter Glotfelty
- strum_macros 0.28.0: Copyright (c) 2019 Peter Glotfelty
- subtle 2.6.1: Copyright (c) 2016-2017 Isis Agora Lovecruft, Henry de Valence. All rights reserved.
- subtle 2.6.1: Copyright (c) 2016-2024 Isis Agora Lovecruft. All rights reserved.
- swift-rs 1.0.8: Copyright (c) 2023 The swift-rs Developers
- synstructure 0.14.0: Copyright 2016 Nika Layzell
- system-configuration 0.8.0: Copyright (c) 2024 Mullvad VPN AB
- system-configuration-sys 0.6.0: Copyright (c) 2024 Mullvad VPN AB
- tauri 2.12.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-codegen 2.7.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-macros 2.7.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-plugin-dialog 2.8.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-plugin-fs 2.6.0: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-runtime 2.12.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-runtime-wry 2.12.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-utils 2.10.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tendril 0.5.1: Copyright (c) 2015 Keegan McAllister
- time 0.3.55: Copyright (c) Jacob Pratt et al.
- time-core 0.1.9: Copyright (c) Jacob Pratt et al.
- time-macros 0.2.32: Copyright (c) Jacob Pratt et al.
- tinystr 0.8.4: Copyright © 2020-2024 Unicode, Inc.
- tinyvec 1.13.3: Copyright (c) 2019 Daniel "Lokathor" Gee.
- tokio 1.53.2: Copyright (c) Tokio Contributors
- tokio-macros 2.7.2: Copyright (c) 2019 Yoshua Wuyts
- tokio-macros 2.7.2: Copyright (c) Tokio Contributors
- tokio-rustls 0.26.6: Copyright (c) 2017 quininer kel
- tokio-stream 0.1.19: Copyright (c) Tokio Contributors
- tokio-tungstenite 0.30.0: Copyright (c) 2017 Daniel Abramov
- tokio-tungstenite 0.30.0: Copyright (c) 2017 Alexey Galakhov
- tokio-util 0.7.19: Copyright (c) Tokio Contributors
- tokio-websockets 0.13.3: Copyright (c) 2021 Jens Reidel
- toml 0.8.2: Copyright (c) Individual contributors
- toml 1.1.6+spec-1.1.0: Copyright (c) Individual contributors
- toml_datetime 0.6.3: Copyright (c) 2014 Alex Crichton
- toml_datetime 1.1.1+spec-1.1.0: Copyright (c) Individual contributors
- toml_edit 0.20.2: Copyright (c) Individual contributors
- toml_edit 0.25.15+spec-1.1.0: Copyright (c) Individual contributors
- toml_parser 1.1.3+spec-1.1.0: Copyright (c) Individual contributors
- toml_writer 1.1.2+spec-1.1.0: Copyright (c) Individual contributors
- tower 0.5.3: Copyright (c) 2019 Tower Contributors
- tower-http 0.6.11: Copyright (c) 2019-2021 Tower Contributors
- tower-layer 0.3.3: Copyright (c) 2019 Tower Contributors
- tower-service 0.3.3: Copyright (c) 2019 Tower Contributors
- tracing 0.1.44: Copyright (c) 2019 Tokio Contributors
- tracing-attributes 0.1.31: Copyright (c) 2019 Tokio Contributors
- tracing-core 0.1.36: Copyright (c) 2019 Tokio Contributors
- trash 5.2.9: Copyright 2019 Artúr Barnabás Kovács
- tray-icon 0.25.1: Copyright (c) 2022-2022 Tauri Programme within The Commons Conservancy
- try-lock 0.2.5: Copyright (c) 2018-2023 Sean McArthur
- try-lock 0.2.5: Copyright (c) 2016 Alex Crichton
- tungstenite 0.30.0: Copyright (c) 2017 Alexey Galakhov
- tungstenite 0.30.0: Copyright (c) 2016 Jason Housley
- typenum 1.20.1: Copyright (c) 2014 Paho Lurie-Gregg
- unicode-ident 1.0.26: Copyright © 1991-2023 Unicode, Inc.
- unicode-segmentation 1.13.3: Copyright (c) 2015 The Rust Project Developers
- unicode-xid 0.2.6: Copyright (c) 2015 The Rust Project Developers
- universal-hash 0.5.1: Copyright (c) 2019-2020 RustCrypto Developers
- url 2.5.8: Copyright (c) 2013-2025 The rust-url developers
- urlpattern 0.6.0: Copyright (c) 2021 the Deno authors
- utf8_iter 1.0.4: Copyright Mozilla Foundation
- uuid 1.27.0: Copyright (c) 2014 The Rust Project Developers
- uuid 1.27.0: Copyright (c) 2018 Ashley Mannix, Christopher Armstrong, Dylan DPC, Hunar Roop Kahlon
- walkdir 2.5.0: Copyright (c) 2015 Andrew Gallant
- want 0.3.2: Copyright (c) 2018-2019 Sean McArthur
- web-time 1.1.0: Copyright (c) 2023 dAxpeDDa
- web_atoms 0.2.6: Copyright (c) 2014 The html5ever Project Developers
- winapi-util 0.1.11: Copyright (c) 2017 Andrew Gallant
- window-vibrancy 0.8.1: Copyright (c) 2020-2022 Tauri Programme within The Commons Conservancy
- windows 0.62.2: Copyright (c) Microsoft Corporation.
- windows-collections 0.3.2: Copyright (c) Microsoft Corporation.
- windows-core 0.62.2: Copyright (c) Microsoft Corporation.
- windows-future 0.3.2: Copyright (c) Microsoft Corporation.
- windows-implement 0.60.2: Copyright (c) Microsoft Corporation.
- windows-interface 0.59.3: Copyright (c) Microsoft Corporation.
- windows-link 0.2.1: Copyright (c) Microsoft Corporation.
- windows-numerics 0.3.1: Copyright (c) Microsoft Corporation.
- windows-registry 0.6.1: Copyright (c) Microsoft Corporation.
- windows-result 0.4.1: Copyright (c) Microsoft Corporation.
- windows-strings 0.5.1: Copyright (c) Microsoft Corporation.
- windows-sys 0.60.2: Copyright (c) Microsoft Corporation.
- windows-sys 0.61.2: Copyright (c) Microsoft Corporation.
- windows-targets 0.53.5: Copyright (c) Microsoft Corporation.
- windows-threading 0.2.1: Copyright (c) Microsoft Corporation.
- windows-version 0.1.7: Copyright (c) Microsoft Corporation.
- windows_x86_64_msvc 0.53.1: Copyright (c) Microsoft Corporation.
- writeable 0.6.4: Copyright © 2020-2024 Unicode, Inc.
- wry 0.57.0: Copyright (c) 2020-2023 Ngo Iok Ui & Tauri Programme within The Commons Conservancy
- xml-rs 0.8.29: Copyright (c) 2014 Vladimir Matveev
- xmltree 0.10.3: Copyright (c) 2015 Andrew Chin
- yoke 0.8.3: Copyright © 2020-2024 Unicode, Inc.
- yoke-derive 0.8.4: Copyright © 2020-2024 Unicode, Inc.
- zerofrom 0.1.8: Copyright © 2020-2024 Unicode, Inc.
- zerofrom-derive 0.1.8: Copyright © 2020-2024 Unicode, Inc.
- zeroize 1.9.1: Copyright (c) 2018-2026 The RustCrypto Project Developers
- zeroize_derive 1.5.0: Copyright (c) 2019-2026 The RustCrypto Project Developers
- zerotrie 0.2.5: Copyright © 2020-2024 Unicode, Inc.
- zerovec 0.11.8: Copyright © 2020-2024 Unicode, Inc.
- zerovec-derive 0.11.6: Copyright © 2020-2024 Unicode, Inc.
- zip 2.4.2: Copyright (c) 2014 Mathijs van de Nes
- zlib-rs 0.6.8: (C) 2024 Trifecta Tech Foundation

Paquetes sin fichero de licencia en el paquete descargado (28). Su aviso de copyright no se pudo leer en local: hay que mirarlo en su repositorio antes de distribuir:

- alloc-stdlib 0.3.0 (BSD-3-Clause)
- block2 0.6.2 (MIT)
- defmt-parser 1.0.0 (MIT OR Apache-2.0)
- dispatch2 0.3.1 (Zlib OR Apache-2.0 OR MIT)
- enum-assoc 1.4.1 (MIT OR Apache-2.0)
- iroh-base 1.3.0 (MIT OR Apache-2.0)
- iroh-dns 1.3.0 (MIT OR Apache-2.0)
- iroh-metrics-derive 1.0.1 (MIT OR Apache-2.0)
- n0-error-macros 1.0.1 (MIT OR Apache-2.0)
- netwatch 0.19.3 (MIT OR Apache-2.0)
- objc2 0.6.5 (MIT)
- objc2-app-kit 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- objc2-core-foundation 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- objc2-core-graphics 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- objc2-encode 4.1.0 (MIT)
- objc2-exception-helper 0.1.1 (Zlib OR Apache-2.0 OR MIT)
- objc2-foundation 0.3.2 (MIT)
- objc2-io-surface 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- objc2-quartz-core 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- objc2-security 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- objc2-system-configuration 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- objc2-user-notifications 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- objc2-web-kit 0.3.2 (Zlib OR Apache-2.0 OR MIT)
- portmapper 0.19.3 (MIT OR Apache-2.0)
- selectors 0.38.0 (MPL-2.0)
- webview2-com 0.39.1 (MIT)
- webview2-com-macros 0.8.1 (MIT)
- webview2-com-sys 0.39.1 (MIT)

## 4. Textos de las licencias

Textos oficiales, sin cambios: SPDX (spdx.org), salvo Apache-2.0 (apache.org) y MPL-2.0 (mozilla.org). Los campos entre < > de las plantillas (año, titular) se rellenan con los avisos de la sección 3.

### 0BSD

Usada por: 1 paquete de Rust.

````text
Copyright (C) YEAR by AUTHOR EMAIL

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
````

### Apache-2.0

Usada por: 310 paquetes de Rust.

````text
Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/

   TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

   1. Definitions.

      "License" shall mean the terms and conditions for use, reproduction,
      and distribution as defined by Sections 1 through 9 of this document.

      "Licensor" shall mean the copyright owner or entity authorized by
      the copyright owner that is granting the License.

      "Legal Entity" shall mean the union of the acting entity and all
      other entities that control, are controlled by, or are under common
      control with that entity. For the purposes of this definition,
      "control" means (i) the power, direct or indirect, to cause the
      direction or management of such entity, whether by contract or
      otherwise, or (ii) ownership of fifty percent (50%) or more of the
      outstanding shares, or (iii) beneficial ownership of such entity.

      "You" (or "Your") shall mean an individual or Legal Entity
      exercising permissions granted by this License.

      "Source" form shall mean the preferred form for making modifications,
      including but not limited to software source code, documentation
      source, and configuration files.

      "Object" form shall mean any form resulting from mechanical
      transformation or translation of a Source form, including but
      not limited to compiled object code, generated documentation,
      and conversions to other media types.

      "Work" shall mean the work of authorship, whether in Source or
      Object form, made available under the License, as indicated by a
      copyright notice that is included in or attached to the work
      (an example is provided in the Appendix below).

      "Derivative Works" shall mean any work, whether in Source or Object
      form, that is based on (or derived from) the Work and for which the
      editorial revisions, annotations, elaborations, or other modifications
      represent, as a whole, an original work of authorship. For the purposes
      of this License, Derivative Works shall not include works that remain
      separable from, or merely link (or bind by name) to the interfaces of,
      the Work and Derivative Works thereof.

      "Contribution" shall mean any work of authorship, including
      the original version of the Work and any modifications or additions
      to that Work or Derivative Works thereof, that is intentionally
      submitted to Licensor for inclusion in the Work by the copyright owner
      or by an individual or Legal Entity authorized to submit on behalf of
      the copyright owner. For the purposes of this definition, "submitted"
      means any form of electronic, verbal, or written communication sent
      to the Licensor or its representatives, including but not limited to
      communication on electronic mailing lists, source code control systems,
      and issue tracking systems that are managed by, or on behalf of, the
      Licensor for the purpose of discussing and improving the Work, but
      excluding communication that is conspicuously marked or otherwise
      designated in writing by the copyright owner as "Not a Contribution."

      "Contributor" shall mean Licensor and any individual or Legal Entity
      on behalf of whom a Contribution has been received by Licensor and
      subsequently incorporated within the Work.

   2. Grant of Copyright License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      copyright license to reproduce, prepare Derivative Works of,
      publicly display, publicly perform, sublicense, and distribute the
      Work and such Derivative Works in Source or Object form.

   3. Grant of Patent License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      (except as stated in this section) patent license to make, have made,
      use, offer to sell, sell, import, and otherwise transfer the Work,
      where such license applies only to those patent claims licensable
      by such Contributor that are necessarily infringed by their
      Contribution(s) alone or by combination of their Contribution(s)
      with the Work to which such Contribution(s) was submitted. If You
      institute patent litigation against any entity (including a
      cross-claim or counterclaim in a lawsuit) alleging that the Work
      or a Contribution incorporated within the Work constitutes direct
      or contributory patent infringement, then any patent licenses
      granted to You under this License for that Work shall terminate
      as of the date such litigation is filed.

   4. Redistribution. You may reproduce and distribute copies of the
      Work or Derivative Works thereof in any medium, with or without
      modifications, and in Source or Object form, provided that You
      meet the following conditions:

      (a) You must give any other recipients of the Work or
          Derivative Works a copy of this License; and

      (b) You must cause any modified files to carry prominent notices
          stating that You changed the files; and

      (c) You must retain, in the Source form of any Derivative Works
          that You distribute, all copyright, patent, trademark, and
          attribution notices from the Source form of the Work,
          excluding those notices that do not pertain to any part of
          the Derivative Works; and

      (d) If the Work includes a "NOTICE" text file as part of its
          distribution, then any Derivative Works that You distribute must
          include a readable copy of the attribution notices contained
          within such NOTICE file, excluding those notices that do not
          pertain to any part of the Derivative Works, in at least one
          of the following places: within a NOTICE text file distributed
          as part of the Derivative Works; within the Source form or
          documentation, if provided along with the Derivative Works; or,
          within a display generated by the Derivative Works, if and
          wherever such third-party notices normally appear. The contents
          of the NOTICE file are for informational purposes only and
          do not modify the License. You may add Your own attribution
          notices within Derivative Works that You distribute, alongside
          or as an addendum to the NOTICE text from the Work, provided
          that such additional attribution notices cannot be construed
          as modifying the License.

      You may add Your own copyright statement to Your modifications and
      may provide additional or different license terms and conditions
      for use, reproduction, or distribution of Your modifications, or
      for any such Derivative Works as a whole, provided Your use,
      reproduction, and distribution of the Work otherwise complies with
      the conditions stated in this License.

   5. Submission of Contributions. Unless You explicitly state otherwise,
      any Contribution intentionally submitted for inclusion in the Work
      by You to the Licensor shall be under the terms and conditions of
      this License, without any additional terms or conditions.
      Notwithstanding the above, nothing herein shall supersede or modify
      the terms of any separate license agreement you may have executed
      with Licensor regarding such Contributions.

   6. Trademarks. This License does not grant permission to use the trade
      names, trademarks, service marks, or product names of the Licensor,
      except as required for reasonable and customary use in describing the
      origin of the Work and reproducing the content of the NOTICE file.

   7. Disclaimer of Warranty. Unless required by applicable law or
      agreed to in writing, Licensor provides the Work (and each
      Contributor provides its Contributions) on an "AS IS" BASIS,
      WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
      implied, including, without limitation, any warranties or conditions
      of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A
      PARTICULAR PURPOSE. You are solely responsible for determining the
      appropriateness of using or redistributing the Work and assume any
      risks associated with Your exercise of permissions under this License.

   8. Limitation of Liability. In no event and under no legal theory,
      whether in tort (including negligence), contract, or otherwise,
      unless required by applicable law (such as deliberate and grossly
      negligent acts) or agreed to in writing, shall any Contributor be
      liable to You for damages, including any direct, indirect, special,
      incidental, or consequential damages of any character arising as a
      result of this License or out of the use or inability to use the
      Work (including but not limited to damages for loss of goodwill,
      work stoppage, computer failure or malfunction, or any and all
      other commercial damages or losses), even if such Contributor
      has been advised of the possibility of such damages.

   9. Accepting Warranty or Additional Liability. While redistributing
      the Work or Derivative Works thereof, You may choose to offer,
      and charge a fee for, acceptance of support, warranty, indemnity,
      or other liability obligations and/or rights consistent with this
      License. However, in accepting such obligations, You may act only
      on Your own behalf and on Your sole responsibility, not on behalf
      of any other Contributor, and only if You agree to indemnify,
      defend, and hold each Contributor harmless for any liability
      incurred by, or claims asserted against, such Contributor by reason
      of your accepting any such warranty or additional liability.

   END OF TERMS AND CONDITIONS

   APPENDIX: How to apply the Apache License to your work.

      To apply the Apache License to your work, attach the following
      boilerplate notice, with the fields enclosed by brackets "[]"
      replaced with your own identifying information. (Don't include
      the brackets!)  The text should be enclosed in the appropriate
      comment syntax for the file format. We also recommend that a
      file or class name and description of purpose be included on the
      same "printed page" as the copyright notice for easier
      identification within third-party archives.

   Copyright [yyyy] [name of copyright owner]

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
````

### BSD-2-Clause

Usada por: 1 paquete de Rust.

````text
Copyright (c) <year> <owner> 

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
````

### BSD-3-Clause

Usada por: 10 paquetes de Rust.

````text
Copyright (c) <year> <owner>. 

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
````

### BSL-1.0

Usada por: 1 paquete de Rust.

````text
Boost Software License - Version 1.0 - August 17th, 2003

Permission is hereby granted, free of charge, to any person or organization obtaining a copy of the software and accompanying documentation covered by this license (the "Software") to use, reproduce, display, distribute, execute, and transmit the Software, and to prepare derivative works of the Software, and to permit third-parties to whom the Software is furnished to do so, all subject to the following:

The copyright notices in the Software and this entire statement, including the above license grant, this restriction and the following disclaimer, must be included in all copies of the Software, in whole or in part, and all derivative works of the Software, unless such copies or derivative works are solely in the form of machine-executable object code generated by a source language processor.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, TITLE AND NON-INFRINGEMENT. IN NO EVENT SHALL THE COPYRIGHT HOLDERS OR ANYONE DISTRIBUTING THE SOFTWARE BE LIABLE FOR ANY DAMAGES OR OTHER LIABILITY, WHETHER IN CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
````

### CC0-1.0

Usada por: 3 paquetes de Rust.

````text
Creative Commons Legal Code

CC0 1.0 Universal

    CREATIVE COMMONS CORPORATION IS NOT A LAW FIRM AND DOES NOT PROVIDE
    LEGAL SERVICES. DISTRIBUTION OF THIS DOCUMENT DOES NOT CREATE AN
    ATTORNEY-CLIENT RELATIONSHIP. CREATIVE COMMONS PROVIDES THIS
    INFORMATION ON AN "AS-IS" BASIS. CREATIVE COMMONS MAKES NO WARRANTIES
    REGARDING THE USE OF THIS DOCUMENT OR THE INFORMATION OR WORKS
    PROVIDED HEREUNDER, AND DISCLAIMS LIABILITY FOR DAMAGES RESULTING FROM
    THE USE OF THIS DOCUMENT OR THE INFORMATION OR WORKS PROVIDED
    HEREUNDER.

Statement of Purpose

The laws of most jurisdictions throughout the world automatically confer
exclusive Copyright and Related Rights (defined below) upon the creator
and subsequent owner(s) (each and all, an "owner") of an original work of
authorship and/or a database (each, a "Work").

Certain owners wish to permanently relinquish those rights to a Work for
the purpose of contributing to a commons of creative, cultural and
scientific works ("Commons") that the public can reliably and without fear
of later claims of infringement build upon, modify, incorporate in other
works, reuse and redistribute as freely as possible in any form whatsoever
and for any purposes, including without limitation commercial purposes.
These owners may contribute to the Commons to promote the ideal of a free
culture and the further production of creative, cultural and scientific
works, or to gain reputation or greater distribution for their Work in
part through the use and efforts of others.

For these and/or other purposes and motivations, and without any
expectation of additional consideration or compensation, the person
associating CC0 with a Work (the "Affirmer"), to the extent that he or she
is an owner of Copyright and Related Rights in the Work, voluntarily
elects to apply CC0 to the Work and publicly distribute the Work under its
terms, with knowledge of his or her Copyright and Related Rights in the
Work and the meaning and intended legal effect of CC0 on those rights.

1. Copyright and Related Rights. A Work made available under CC0 may be
protected by copyright and related or neighboring rights ("Copyright and
Related Rights"). Copyright and Related Rights include, but are not
limited to, the following:

  i. the right to reproduce, adapt, distribute, perform, display,
     communicate, and translate a Work;
 ii. moral rights retained by the original author(s) and/or performer(s);
iii. publicity and privacy rights pertaining to a person's image or
     likeness depicted in a Work;
 iv. rights protecting against unfair competition in regards to a Work,
     subject to the limitations in paragraph 4(a), below;
  v. rights protecting the extraction, dissemination, use and reuse of data
     in a Work;
 vi. database rights (such as those arising under Directive 96/9/EC of the
     European Parliament and of the Council of 11 March 1996 on the legal
     protection of databases, and under any national implementation
     thereof, including any amended or successor version of such
     directive); and
vii. other similar, equivalent or corresponding rights throughout the
     world based on applicable law or treaty, and any national
     implementations thereof.

2. Waiver. To the greatest extent permitted by, but not in contravention
of, applicable law, Affirmer hereby overtly, fully, permanently,
irrevocably and unconditionally waives, abandons, and surrenders all of
Affirmer's Copyright and Related Rights and associated claims and causes
of action, whether now known or unknown (including existing as well as
future claims and causes of action), in the Work (i) in all territories
worldwide, (ii) for the maximum duration provided by applicable law or
treaty (including future time extensions), (iii) in any current or future
medium and for any number of copies, and (iv) for any purpose whatsoever,
including without limitation commercial, advertising or promotional
purposes (the "Waiver"). Affirmer makes the Waiver for the benefit of each
member of the public at large and to the detriment of Affirmer's heirs and
successors, fully intending that such Waiver shall not be subject to
revocation, rescission, cancellation, termination, or any other legal or
equitable action to disrupt the quiet enjoyment of the Work by the public
as contemplated by Affirmer's express Statement of Purpose.

3. Public License Fallback. Should any part of the Waiver for any reason
be judged legally invalid or ineffective under applicable law, then the
Waiver shall be preserved to the maximum extent permitted taking into
account Affirmer's express Statement of Purpose. In addition, to the
extent the Waiver is so judged Affirmer hereby grants to each affected
person a royalty-free, non transferable, non sublicensable, non exclusive,
irrevocable and unconditional license to exercise Affirmer's Copyright and
Related Rights in the Work (i) in all territories worldwide, (ii) for the
maximum duration provided by applicable law or treaty (including future
time extensions), (iii) in any current or future medium and for any number
of copies, and (iv) for any purpose whatsoever, including without
limitation commercial, advertising or promotional purposes (the
"License"). The License shall be deemed effective as of the date CC0 was
applied by Affirmer to the Work. Should any part of the License for any
reason be judged legally invalid or ineffective under applicable law, such
partial invalidity or ineffectiveness shall not invalidate the remainder
of the License, and in such case Affirmer hereby affirms that he or she
will not (i) exercise any of his or her remaining Copyright and Related
Rights in the Work or (ii) assert any associated claims and causes of
action with respect to the Work, in either case contrary to Affirmer's
express Statement of Purpose.

4. Limitations and Disclaimers.

 a. No trademark or patent rights held by Affirmer are waived, abandoned,
    surrendered, licensed or otherwise affected by this document.
 b. Affirmer offers the Work as-is and makes no representations or
    warranties of any kind concerning the Work, express, implied,
    statutory or otherwise, including without limitation warranties of
    title, merchantability, fitness for a particular purpose, non
    infringement, or the absence of latent or other defects, accuracy, or
    the present or absence of errors, whether or not discoverable, all to
    the greatest extent permissible under applicable law.
 c. Affirmer disclaims responsibility for clearing rights of other persons
    that may apply to the Work or any use thereof, including without
    limitation any person's Copyright and Related Rights in the Work.
    Further, Affirmer disclaims responsibility for obtaining any necessary
    consents, permissions or other rights required for any use of the
    Work.
 d. Affirmer understands and acknowledges that Creative Commons is not a
    party to this document and has no duty or obligation with respect to
    this CC0 or use of the Work.
````

### CDLA-Permissive-2.0

Usada por: 2 paquetes de Rust.

````text
Community Data License Agreement - Permissive - Version 2.0

This is the Community Data License Agreement - Permissive, Version 2.0 (the "agreement"). Data Provider(s) and Data Recipient(s) agree as follows:

1. Provision of the Data

1.1. A Data Recipient may use, modify, and share the Data made available by Data Provider(s) under this agreement if that Data Recipient follows the terms of this agreement.

1.2. This agreement does not impose any restriction on a Data Recipient's use, modification, or sharing of any portions of the Data that are in the public domain or that may be used, modified, or shared under any other legal exception or limitation.

2. Conditions for Sharing Data

2.1. A Data Recipient may share Data, with or without modifications, so long as the Data Recipient makes available the text of this agreement with the shared Data.

3. No Restrictions on Results

3.1. This agreement does not impose any restriction or obligations with respect to the use, modification, or sharing of Results.

4. No Warranty; Limitation of Liability

4.1. All Data Recipients receive the Data subject to the following terms:

THE DATA IS PROVIDED ON AN "AS IS" BASIS, WITHOUT REPRESENTATIONS, WARRANTIES OR CONDITIONS OF ANY KIND, EITHER EXPRESS OR IMPLIED INCLUDING, WITHOUT LIMITATION, ANY WARRANTIES OR CONDITIONS OF TITLE, NON-INFRINGEMENT, MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.

NO DATA PROVIDER SHALL HAVE ANY LIABILITY FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING WITHOUT LIMITATION LOST PROFITS), HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE DATA OR RESULTS, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGES.

5. Definitions

5.1. "Data" means the material received by a Data Recipient under this agreement.

5.2. "Data Provider" means any person who is the source of Data provided under this agreement and in reliance on a Data Recipient's agreement to its terms.

5.3. "Data Recipient" means any person who receives Data directly or indirectly from a Data Provider and agrees to the terms of this agreement.

5.4. "Results" means any outcome obtained by computational analysis of Data, including for example machine learning models and models' insights.
````

### ISC

Usada por: 5 paquetes de Rust.

````text
ISC License

Copyright <year> <owner> 

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
````

### LLVM-exception

Usada por: 1 paquete de Rust.

````text
---- LLVM Exceptions to the Apache 2.0 License ----

   As an exception, if, as a result of your compiling your source code, portions
   of this Software are embedded into an Object form of such source code, you
   may redistribute such embedded portions in such Object form without complying
   with the conditions of Sections 4(a), 4(b) and 4(d) of the License.

   In addition, if you combine or link compiled forms of this Software with
   software that is licensed under the GPLv2 ("Combined Software") and if a
   court of competent jurisdiction determines that the patent provision (Section
   3), the indemnity provision (Section 9) or other Section of the License
   conflicts with the conditions of the GPLv2, you may retroactively and
   prospectively choose to deem waived or otherwise exclude such Section(s) of
   the License, but only in their entirety and only with respect to the Combined
   Software.
````

### MIT

Usada por: 394 paquetes de Rust.

````text
MIT License

Copyright (c) <year> <copyright holders>

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the
following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT
LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
USE OR OTHER DEALINGS IN THE SOFTWARE.
````

### MIT-0

Usada por: 2 paquetes de Rust.

````text
MIT No Attribution

Copyright <YEAR> <COPYRIGHT HOLDER>

Permission is hereby granted, free of charge, to any person obtaining a copy of this
software and associated documentation files (the "Software"), to deal in the Software
without restriction, including without limitation the rights to use, copy, modify,
merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
````

### MPL-2.0

Usada por: 6 paquetes de Rust.

````text
Mozilla Public License Version 2.0
==================================

1. Definitions
--------------

1.1. "Contributor"
    means each individual or legal entity that creates, contributes to
    the creation of, or owns Covered Software.

1.2. "Contributor Version"
    means the combination of the Contributions of others (if any) used
    by a Contributor and that particular Contributor's Contribution.

1.3. "Contribution"
    means Covered Software of a particular Contributor.

1.4. "Covered Software"
    means Source Code Form to which the initial Contributor has attached
    the notice in Exhibit A, the Executable Form of such Source Code
    Form, and Modifications of such Source Code Form, in each case
    including portions thereof.

1.5. "Incompatible With Secondary Licenses"
    means

    (a) that the initial Contributor has attached the notice described
        in Exhibit B to the Covered Software; or

    (b) that the Covered Software was made available under the terms of
        version 1.1 or earlier of the License, but not also under the
        terms of a Secondary License.

1.6. "Executable Form"
    means any form of the work other than Source Code Form.

1.7. "Larger Work"
    means a work that combines Covered Software with other material, in
    a separate file or files, that is not Covered Software.

1.8. "License"
    means this document.

1.9. "Licensable"
    means having the right to grant, to the maximum extent possible,
    whether at the time of the initial grant or subsequently, any and
    all of the rights conveyed by this License.

1.10. "Modifications"
    means any of the following:

    (a) any file in Source Code Form that results from an addition to,
        deletion from, or modification of the contents of Covered
        Software; or

    (b) any new file in Source Code Form that contains any Covered
        Software.

1.11. "Patent Claims" of a Contributor
    means any patent claim(s), including without limitation, method,
    process, and apparatus claims, in any patent Licensable by such
    Contributor that would be infringed, but for the grant of the
    License, by the making, using, selling, offering for sale, having
    made, import, or transfer of either its Contributions or its
    Contributor Version.

1.12. "Secondary License"
    means either the GNU General Public License, Version 2.0, the GNU
    Lesser General Public License, Version 2.1, the GNU Affero General
    Public License, Version 3.0, or any later versions of those
    licenses.

1.13. "Source Code Form"
    means the form of the work preferred for making modifications.

1.14. "You" (or "Your")
    means an individual or a legal entity exercising rights under this
    License. For legal entities, "You" includes any entity that
    controls, is controlled by, or is under common control with You. For
    purposes of this definition, "control" means (a) the power, direct
    or indirect, to cause the direction or management of such entity,
    whether by contract or otherwise, or (b) ownership of more than
    fifty percent (50%) of the outstanding shares or beneficial
    ownership of such entity.

2. License Grants and Conditions
--------------------------------

2.1. Grants

Each Contributor hereby grants You a world-wide, royalty-free,
non-exclusive license:

(a) under intellectual property rights (other than patent or trademark)
    Licensable by such Contributor to use, reproduce, make available,
    modify, display, perform, distribute, and otherwise exploit its
    Contributions, either on an unmodified basis, with Modifications, or
    as part of a Larger Work; and

(b) under Patent Claims of such Contributor to make, use, sell, offer
    for sale, have made, import, and otherwise transfer either its
    Contributions or its Contributor Version.

2.2. Effective Date

The licenses granted in Section 2.1 with respect to any Contribution
become effective for each Contribution on the date the Contributor first
distributes such Contribution.

2.3. Limitations on Grant Scope

The licenses granted in this Section 2 are the only rights granted under
this License. No additional rights or licenses will be implied from the
distribution or licensing of Covered Software under this License.
Notwithstanding Section 2.1(b) above, no patent license is granted by a
Contributor:

(a) for any code that a Contributor has removed from Covered Software;
    or

(b) for infringements caused by: (i) Your and any other third party's
    modifications of Covered Software, or (ii) the combination of its
    Contributions with other software (except as part of its Contributor
    Version); or

(c) under Patent Claims infringed by Covered Software in the absence of
    its Contributions.

This License does not grant any rights in the trademarks, service marks,
or logos of any Contributor (except as may be necessary to comply with
the notice requirements in Section 3.4).

2.4. Subsequent Licenses

No Contributor makes additional grants as a result of Your choice to
distribute the Covered Software under a subsequent version of this
License (see Section 10.2) or under the terms of a Secondary License (if
permitted under the terms of Section 3.3).

2.5. Representation

Each Contributor represents that the Contributor believes its
Contributions are its original creation(s) or it has sufficient rights
to grant the rights to its Contributions conveyed by this License.

2.6. Fair Use

This License is not intended to limit any rights You have under
applicable copyright doctrines of fair use, fair dealing, or other
equivalents.

2.7. Conditions

Sections 3.1, 3.2, 3.3, and 3.4 are conditions of the licenses granted
in Section 2.1.

3. Responsibilities
-------------------

3.1. Distribution of Source Form

All distribution of Covered Software in Source Code Form, including any
Modifications that You create or to which You contribute, must be under
the terms of this License. You must inform recipients that the Source
Code Form of the Covered Software is governed by the terms of this
License, and how they can obtain a copy of this License. You may not
attempt to alter or restrict the recipients' rights in the Source Code
Form.

3.2. Distribution of Executable Form

If You distribute Covered Software in Executable Form then:

(a) such Covered Software must also be made available in Source Code
    Form, as described in Section 3.1, and You must inform recipients of
    the Executable Form how they can obtain a copy of such Source Code
    Form by reasonable means in a timely manner, at a charge no more
    than the cost of distribution to the recipient; and

(b) You may distribute such Executable Form under the terms of this
    License, or sublicense it under different terms, provided that the
    license for the Executable Form does not attempt to limit or alter
    the recipients' rights in the Source Code Form under this License.

3.3. Distribution of a Larger Work

You may create and distribute a Larger Work under terms of Your choice,
provided that You also comply with the requirements of this License for
the Covered Software. If the Larger Work is a combination of Covered
Software with a work governed by one or more Secondary Licenses, and the
Covered Software is not Incompatible With Secondary Licenses, this
License permits You to additionally distribute such Covered Software
under the terms of such Secondary License(s), so that the recipient of
the Larger Work may, at their option, further distribute the Covered
Software under the terms of either this License or such Secondary
License(s).

3.4. Notices

You may not remove or alter the substance of any license notices
(including copyright notices, patent notices, disclaimers of warranty,
or limitations of liability) contained within the Source Code Form of
the Covered Software, except that You may alter any license notices to
the extent required to remedy known factual inaccuracies.

3.5. Application of Additional Terms

You may choose to offer, and to charge a fee for, warranty, support,
indemnity or liability obligations to one or more recipients of Covered
Software. However, You may do so only on Your own behalf, and not on
behalf of any Contributor. You must make it absolutely clear that any
such warranty, support, indemnity, or liability obligation is offered by
You alone, and You hereby agree to indemnify every Contributor for any
liability incurred by such Contributor as a result of warranty, support,
indemnity or liability terms You offer. You may include additional
disclaimers of warranty and limitations of liability specific to any
jurisdiction.

4. Inability to Comply Due to Statute or Regulation
---------------------------------------------------

If it is impossible for You to comply with any of the terms of this
License with respect to some or all of the Covered Software due to
statute, judicial order, or regulation then You must: (a) comply with
the terms of this License to the maximum extent possible; and (b)
describe the limitations and the code they affect. Such description must
be placed in a text file included with all distributions of the Covered
Software under this License. Except to the extent prohibited by statute
or regulation, such description must be sufficiently detailed for a
recipient of ordinary skill to be able to understand it.

5. Termination
--------------

5.1. The rights granted under this License will terminate automatically
if You fail to comply with any of its terms. However, if You become
compliant, then the rights granted under this License from a particular
Contributor are reinstated (a) provisionally, unless and until such
Contributor explicitly and finally terminates Your grants, and (b) on an
ongoing basis, if such Contributor fails to notify You of the
non-compliance by some reasonable means prior to 60 days after You have
come back into compliance. Moreover, Your grants from a particular
Contributor are reinstated on an ongoing basis if such Contributor
notifies You of the non-compliance by some reasonable means, this is the
first time You have received notice of non-compliance with this License
from such Contributor, and You become compliant prior to 30 days after
Your receipt of the notice.

5.2. If You initiate litigation against any entity by asserting a patent
infringement claim (excluding declaratory judgment actions,
counter-claims, and cross-claims) alleging that a Contributor Version
directly or indirectly infringes any patent, then the rights granted to
You by any and all Contributors for the Covered Software under Section
2.1 of this License shall terminate.

5.3. In the event of termination under Sections 5.1 or 5.2 above, all
end user license agreements (excluding distributors and resellers) which
have been validly granted by You or Your distributors under this License
prior to termination shall survive termination.

************************************************************************
*                                                                      *
*  6. Disclaimer of Warranty                                           *
*  -------------------------                                           *
*                                                                      *
*  Covered Software is provided under this License on an "as is"       *
*  basis, without warranty of any kind, either expressed, implied, or  *
*  statutory, including, without limitation, warranties that the       *
*  Covered Software is free of defects, merchantable, fit for a        *
*  particular purpose or non-infringing. The entire risk as to the     *
*  quality and performance of the Covered Software is with You.        *
*  Should any Covered Software prove defective in any respect, You     *
*  (not any Contributor) assume the cost of any necessary servicing,   *
*  repair, or correction. This disclaimer of warranty constitutes an   *
*  essential part of this License. No use of any Covered Software is   *
*  authorized under this License except under this disclaimer.         *
*                                                                      *
************************************************************************

************************************************************************
*                                                                      *
*  7. Limitation of Liability                                          *
*  --------------------------                                          *
*                                                                      *
*  Under no circumstances and under no legal theory, whether tort      *
*  (including negligence), contract, or otherwise, shall any           *
*  Contributor, or anyone who distributes Covered Software as          *
*  permitted above, be liable to You for any direct, indirect,         *
*  special, incidental, or consequential damages of any character      *
*  including, without limitation, damages for lost profits, loss of    *
*  goodwill, work stoppage, computer failure or malfunction, or any    *
*  and all other commercial damages or losses, even if such party      *
*  shall have been informed of the possibility of such damages. This   *
*  limitation of liability shall not apply to liability for death or   *
*  personal injury resulting from such party's negligence to the       *
*  extent applicable law prohibits such limitation. Some               *
*  jurisdictions do not allow the exclusion or limitation of           *
*  incidental or consequential damages, so this exclusion and          *
*  limitation may not apply to You.                                    *
*                                                                      *
************************************************************************

8. Litigation
-------------

Any litigation relating to this License may be brought only in the
courts of a jurisdiction where the defendant maintains its principal
place of business and such litigation shall be governed by laws of that
jurisdiction, without reference to its conflict-of-law provisions.
Nothing in this Section shall prevent a party's ability to bring
cross-claims or counter-claims.

9. Miscellaneous
----------------

This License represents the complete agreement concerning the subject
matter hereof. If any provision of this License is held to be
unenforceable, such provision shall be reformed only to the extent
necessary to make it enforceable. Any law or regulation which provides
that the language of a contract shall be construed against the drafter
shall not be used to construe this License against a Contributor.

10. Versions of the License
---------------------------

10.1. New Versions

Mozilla Foundation is the license steward. Except as provided in Section
10.3, no one other than the license steward has the right to modify or
publish new versions of this License. Each version will be given a
distinguishing version number.

10.2. Effect of New Versions

You may distribute the Covered Software under the terms of the version
of the License under which You originally received the Covered Software,
or under the terms of any subsequent version published by the license
steward.

10.3. Modified Versions

If you create software not governed by this License, and you want to
create a new license for such software, you may create and use a
modified version of this License if you rename the license and remove
any references to the name of the license steward (except to note that
such modified license differs from this License).

10.4. Distributing Source Code Form that is Incompatible With Secondary
Licenses

If You choose to distribute Source Code Form that is Incompatible With
Secondary Licenses under the terms of this version of the License, the
notice described in Exhibit B of this License must be attached.

Exhibit A - Source Code Form License Notice
-------------------------------------------

  This Source Code Form is subject to the terms of the Mozilla Public
  License, v. 2.0. If a copy of the MPL was not distributed with this
  file, You can obtain one at https://mozilla.org/MPL/2.0/.

If it is not possible or desirable to put the notice in a particular
file, then You may include the notice in a location (such as a LICENSE
file in a relevant directory) where a recipient would be likely to look
for such a notice.

You may add additional accurate notices of copyright ownership.

Exhibit B - "Incompatible With Secondary Licenses" Notice
---------------------------------------------------------

  This Source Code Form is "Incompatible With Secondary Licenses", as
  defined by the Mozilla Public License, v. 2.0.
````

### OFL-1.1

Usada por: Inter (fuente).

````text
SIL OPEN FONT LICENSE

Version 1.1 - 26 February 2007

PREAMBLE

The goals of the Open Font License (OFL) are to stimulate worldwide development of collaborative font projects, to support the font creation efforts of academic and linguistic communities, and to provide a free and open framework in which fonts may be shared and improved in partnership with others.

The OFL allows the licensed fonts to be used, studied, modified and redistributed freely as long as they are not sold by themselves. The fonts, including any derivative works, can be bundled, embedded, redistributed and/or sold with any software provided that any reserved names are not used by derivative works. The fonts and derivatives, however, cannot be released under any other type of license. The requirement for fonts to remain under this license does not apply to any document created using the fonts or their derivatives.

DEFINITIONS

"Font Software" refers to the set of files released by the Copyright Holder(s) under this license and clearly marked as such. This may include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the copyright statement(s).

"Original Version" refers to the collection of Font Software components as distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting, or substituting — in part or in whole — any of the components of the Original Version, by changing formats or by porting the Font Software to a new environment.

"Author" refers to any designer, engineer, programmer, technical writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS

Permission is hereby granted, free of charge, to any person obtaining a copy of the Font Software, to use, study, copy, merge, embed, modify, redistribute, and sell modified and unmodified copies of the Font Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components, in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled, redistributed and/or sold with any software, provided that each copy contains the above copyright notice and this license. These can be included either as stand-alone text files, human-readable headers or in the appropriate machine-readable metadata fields within text or binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font Name(s) unless explicit written permission is granted by the corresponding Copyright Holder. This restriction only applies to the primary font name as presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font Software shall not be used to promote, endorse or advertise any Modified Version, except to acknowledge the contribution(s) of the Copyright Holder(s) and the Author(s) or with their explicit written permission.

5) The Font Software, modified or unmodified, in part or in whole, must be distributed entirely under this license, and must not be distributed under any other license. The requirement for fonts to remain under this license does not apply to any document created using the Font Software.

TERMINATION

This license becomes null and void if any of the above conditions are not met.

DISCLAIMER

THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM OTHER DEALINGS IN THE FONT SOFTWARE.
````

### Unicode-3.0

Usada por: 19 paquetes de Rust.

````text
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 1991-2023 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.
````

### Unlicense

Usada por: 10 paquetes de Rust.

````text
This is free and unencumbered software released into the public domain.

Anyone is free to copy, modify, publish, use, compile, sell, or distribute this software, either in source code form or as a compiled binary, for any purpose, commercial or non-commercial, and by any means.

In jurisdictions that recognize copyright laws, the author or authors of this software dedicate any and all copyright interest in the software to the public domain. We make this dedication for the benefit of the public at large and to the detriment of our heirs and
successors. We intend this dedication to be an overt act of relinquishment in perpetuity of all present and future rights to this software under copyright law.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

For more information, please refer to <http://unlicense.org/>
````

### Zlib

Usada por: 18 paquetes de Rust.

````text
zlib License

This software is provided 'as-is', without any express or implied warranty.  In no event will the authors be held liable for any damages arising from the use of this software.

Permission is granted to anyone to use this software for any purpose, including commercial applications, and to alter it and redistribute it freely, subject to the following restrictions:

     1. The origin of this software must not be misrepresented; you must not claim that you wrote the original software. If you use this software in a product, an acknowledgment in the product documentation would be appreciated but is not required.

     2. Altered source versions must be plainly marked as such, and must not be misrepresented as being the original software.

     3. This notice may not be removed or altered from any source distribution.
````
