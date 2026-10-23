# Third-party notices

Cablebag is built with the open-source components below. Each is used under its own licence, unchanged;
their licence texts are in the linked projects. The licence of Cablebag itself (LICENSE) does not limit the
rights these licences give. Components under the Mozilla Public License 2.0 are unchanged; their source code
is available from the linked projects.

The Remote Desktop helper uses only the .NET runtime (MIT) and the Remote Desktop control that is part of
Windows. The window uses Microsoft Edge WebView2, which is part of Windows.

## Programs shipped with Cablebag

To show windows of Linux programs from SSH sessions (X11 forwarding), the Windows version comes with the X server
VcXsrv 21.1.16.1, in the folder vcxsrv next to Cablebag. It is a separate program, shipped
unchanged from its official release: Cablebag only starts it and talks to it over the X protocol. VcXsrv is
free software under the GNU General Public License version 3 (GPL-3.0); its licence text is in
vcxsrv\COPYING, and its components (such as the X.Org server, Mesa, FreeType and OpenSSL) carry their own
notices in its source code. The licence of Cablebag does not apply to it: you may use, copy, change and pass it
on under the terms of the GPL.

The complete corresponding source code of VcXsrv 21.1.16.1 is at https://github.com/marchaesen/vcxsrv/tree/21.1.16.1
(archive: https://github.com/marchaesen/vcxsrv/archive/refs/tags/21.1.16.1.tar.gz). For at least three years after the last Cablebag release that
contains this version, a copy of that source code is also available from the Cablebag maintainer at no charge
beyond the cost of providing it: ask in an issue at https://github.com/maferick/cablebag/issues.

| Program | Version | Licence | Where |
| --- | --- | --- | --- |
| VcXsrv | 21.1.16.1 | GPL-3.0 | https://github.com/marchaesen/vcxsrv |

## Components built into Cablebag

| Component | Version | Licence | Where |
| --- | --- | --- | --- |
| @jridgewell/gen-mapping (JavaScript) | 0.3.13 | MIT | https://www.npmjs.com/package/@jridgewell/gen-mapping |
| @jridgewell/remapping (JavaScript) | 2.3.5 | MIT | https://www.npmjs.com/package/@jridgewell/remapping |
| @jridgewell/resolve-uri (JavaScript) | 3.1.2 | MIT | https://www.npmjs.com/package/@jridgewell/resolve-uri |
| @jridgewell/sourcemap-codec (JavaScript) | 1.6.0 | MIT | https://www.npmjs.com/package/@jridgewell/sourcemap-codec |
| @jridgewell/trace-mapping (JavaScript) | 0.3.31 | MIT | https://www.npmjs.com/package/@jridgewell/trace-mapping |
| @lucide/svelte (JavaScript) | 1.61.0 | ISC | https://www.npmjs.com/package/@lucide/svelte |
| @sveltejs/acorn-typescript (JavaScript) | 1.0.13 | MIT | https://www.npmjs.com/package/@sveltejs/acorn-typescript |
| @tauri-apps/api (JavaScript) | 2.12.4 | Apache-2.0 OR MIT | https://www.npmjs.com/package/@tauri-apps/api |
| @tauri-apps/plugin-dialog (JavaScript) | 2.8.3 | MIT OR Apache-2.0 | https://www.npmjs.com/package/@tauri-apps/plugin-dialog |
| @tauri-apps/plugin-opener (JavaScript) | 2.7.2 | MIT OR Apache-2.0 | https://www.npmjs.com/package/@tauri-apps/plugin-opener |
| @types/estree (JavaScript) | 1.0.9 | MIT | https://www.npmjs.com/package/@types/estree |
| @xterm/addon-fit (JavaScript) | 0.11.0 | MIT | https://www.npmjs.com/package/@xterm/addon-fit |
| @xterm/addon-search (JavaScript) | 0.16.0 | MIT | https://www.npmjs.com/package/@xterm/addon-search |
| @xterm/addon-unicode11 (JavaScript) | 0.9.0 | MIT | https://www.npmjs.com/package/@xterm/addon-unicode11 |
| @xterm/addon-web-links (JavaScript) | 0.12.0 | MIT | https://www.npmjs.com/package/@xterm/addon-web-links |
| @xterm/addon-webgl (JavaScript) | 0.19.0 | MIT | https://www.npmjs.com/package/@xterm/addon-webgl |
| @xterm/xterm (JavaScript) | 6.0.0 | MIT | https://www.npmjs.com/package/@xterm/xterm |
| acorn (JavaScript) | 8.19.0 | MIT | https://www.npmjs.com/package/acorn |
| aria-query (JavaScript) | 5.3.1 | Apache-2.0 | https://www.npmjs.com/package/aria-query |
| axobject-query (JavaScript) | 4.1.0 | Apache-2.0 | https://www.npmjs.com/package/axobject-query |
| clsx (JavaScript) | 2.1.1 | MIT | https://www.npmjs.com/package/clsx |
| devalue (JavaScript) | 5.9.4 | MIT | https://www.npmjs.com/package/devalue |
| esm-env (JavaScript) | 1.2.2 | MIT | https://www.npmjs.com/package/esm-env |
| esrap (JavaScript) | 2.4.0 | MIT | https://www.npmjs.com/package/esrap |
| is-reference (JavaScript) | 3.0.3 | MIT | https://www.npmjs.com/package/is-reference |
| locate-character (JavaScript) | 3.0.0 | MIT | https://www.npmjs.com/package/locate-character |
| magic-string (JavaScript) | 0.30.21 | MIT | https://www.npmjs.com/package/magic-string |
| svelte (JavaScript) | 5.57.2 | MIT | https://www.npmjs.com/package/svelte |
| zimmerframe (JavaScript) | 1.1.5 | MIT | https://www.npmjs.com/package/zimmerframe |
| adler2 (Rust) | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | https://github.com/oyvindln/adler2 |
| aead (Rust) | 0.5.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| aead (Rust) | 0.6.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| aes (Rust) | 0.9.3 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-ciphers |
| aes-gcm (Rust) | 0.11.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/AEADs |
| aho-corasick (Rust) | 1.1.5 | Unlicense OR MIT | https://github.com/BurntSushi/aho-corasick |
| alloc-no-stdlib (Rust) | 3.0.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| alloc-stdlib (Rust) | 0.3.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| anyhow (Rust) | 1.0.104 | MIT OR Apache-2.0 | https://github.com/dtolnay/anyhow |
| argon2 (Rust) | 0.6.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/password-hashes |
| async_io_stream (Rust) | 0.3.3 | Unlicense | https://github.com/najamelan/async_io_stream |
| async-trait (Rust) | 0.1.92 | MIT OR Apache-2.0 | https://github.com/dtolnay/async-trait |
| atomic-waker (Rust) | 1.1.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/atomic-waker |
| autocfg (Rust) | 1.5.1 | Apache-2.0 OR MIT | https://github.com/cuviper/autocfg |
| base16ct (Rust) | 1.0.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| base64 (Rust) | 0.22.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| base64 (Rust) | 0.23.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| base64ct (Rust) | 1.8.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| bcrypt-pbkdf (Rust) | 0.11.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/password-hashes |
| bit-set (Rust) | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-set |
| bit-vec (Rust) | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-vec |
| bitflags (Rust) | 1.3.2 | MIT/Apache-2.0 | https://github.com/bitflags/bitflags |
| bitflags (Rust) | 2.13.2 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags |
| blake2 (Rust) | 0.10.6 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| blake2 (Rust) | 0.11.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| block-buffer (Rust) | 0.10.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| block-buffer (Rust) | 0.12.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| block-padding (Rust) | 0.4.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| blowfish (Rust) | 0.10.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-ciphers |
| boringtun (Rust) | 0.7.1 | BSD-3-Clause | https://github.com/cloudflare/boringtun |
| brotli (Rust) | 9.0.0 | BSD-3-Clause AND MIT | https://github.com/dropbox/rust-brotli |
| brotli-decompressor (Rust) | 6.0.1 | BSD-3-Clause/MIT | https://github.com/dropbox/rust-brotli-decompressor |
| bs58 (Rust) | 0.5.1 | MIT/Apache-2.0 | https://github.com/Nullus157/bs58-rs |
| byteorder (Rust) | 1.5.0 | Unlicense OR MIT | https://github.com/BurntSushi/byteorder |
| bytes (Rust) | 1.12.1 | MIT | https://github.com/tokio-rs/bytes |
| camino (Rust) | 1.2.6 | MIT OR Apache-2.0 | https://github.com/camino-rs/camino |
| cargo_metadata (Rust) | 0.19.2 | MIT | https://github.com/oli-obk/cargo_metadata |
| cargo_toml (Rust) | 1.0.1 | Apache-2.0 OR MIT | https://gitlab.com/lib.rs/cargo_toml |
| cargo-platform (Rust) | 0.1.9 | MIT OR Apache-2.0 | https://github.com/rust-lang/cargo |
| cbc (Rust) | 0.2.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-modes |
| cc (Rust) | 1.9.0 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| cfb (Rust) | 0.14.0 | MIT | https://github.com/mdsteele/rust-cfb |
| cfg_aliases (Rust) | 0.1.1 | MIT | https://github.com/katharostech/cfg_aliases |
| cfg-if (Rust) | 1.0.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/cfg-if |
| chacha20 (Rust) | 0.10.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/stream-ciphers |
| chacha20 (Rust) | 0.9.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/stream-ciphers |
| chacha20poly1305 (Rust) | 0.10.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/AEADs/tree/master/chacha20poly1305 |
| chrono (Rust) | 0.4.45 | MIT OR Apache-2.0 | https://github.com/chronotope/chrono |
| cipher (Rust) | 0.4.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| cipher (Rust) | 0.5.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| cmov (Rust) | 0.5.4 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| const-oid (Rust) | 0.10.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| const-oid (Rust) | 0.9.6 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/const-oid |
| cookie (Rust) | 0.18.2 | MIT OR Apache-2.0 | https://github.com/SergioBenitez/cookie-rs |
| cpubits (Rust) | 0.1.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| cpufeatures (Rust) | 0.2.17 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| cpufeatures (Rust) | 0.3.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| crc32fast (Rust) | 1.5.3 | MIT OR Apache-2.0 | https://github.com/srijs/rust-crc32fast |
| crossbeam-channel (Rust) | 0.5.17 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossbeam-utils (Rust) | 0.8.23 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crypto-bigint (Rust) | 0.7.5 | Apache-2.0 OR MIT | https://github.com/RustCrypto/crypto-bigint |
| crypto-common (Rust) | 0.1.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| crypto-common (Rust) | 0.2.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| crypto-primes (Rust) | 0.7.2 | Apache-2.0 OR MIT | https://github.com/entropyxyz/crypto-primes |
| cssparser (Rust) | 0.37.0 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| cssparser-macros (Rust) | 0.7.1 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| ctor (Rust) | 1.0.13 | Apache-2.0 OR MIT | https://github.com/mmastrac/linktime |
| ctr (Rust) | 0.10.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-modes |
| ctutils (Rust) | 0.4.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| curve25519-dalek (Rust) | 4.1.3 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/curve25519-dalek |
| curve25519-dalek (Rust) | 5.0.0 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/curve25519-dalek |
| curve25519-dalek-derive (Rust) | 0.1.1 | MIT/Apache-2.0 | https://github.com/dalek-cryptography/curve25519-dalek |
| darling (Rust) | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| darling_core (Rust) | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| darling_macro (Rust) | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| dashmap (Rust) | 6.2.1 | MIT | https://github.com/xacrimon/dashmap |
| data-encoding (Rust) | 2.11.1 | MIT | https://github.com/ia0/data-encoding |
| defmt (Rust) | 1.2.0 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| defmt-macros (Rust) | 1.2.0 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| defmt-parser (Rust) | 1.1.0 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| delegate (Rust) | 0.13.5 | MIT OR Apache-2.0 | https://github.com/kobzol/rust-delegate |
| der (Rust) | 0.7.10 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/der |
| der (Rust) | 0.8.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| deranged (Rust) | 0.5.9 | MIT OR Apache-2.0 | https://github.com/jhpratt/deranged |
| derive_more (Rust) | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| derive_more-impl (Rust) | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| des (Rust) | 0.9.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-ciphers |
| digest (Rust) | 0.10.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| digest (Rust) | 0.11.3 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| dirs (Rust) | 7.0.0 | MIT OR Apache-2.0 | https://codeberg.org/dirs/dirs-rs |
| dirs-sys (Rust) | 0.5.0 | MIT OR Apache-2.0 | https://github.com/dirs-dev/dirs-sys-rs |
| displaydoc (Rust) | 0.2.7 | MIT OR Apache-2.0 | https://github.com/yaahc/displaydoc |
| dns-lookup (Rust) | 4.0.2 | MIT OR Apache-2.0 | https://github.com/keeperofdakeys/dns-lookup/ |
| dom_query (Rust) | 0.28.0 | MIT | https://github.com/niklak/dom_query |
| downcast-rs (Rust) | 1.2.1 | MIT/Apache-2.0 | https://github.com/marcianx/downcast-rs |
| dpi (Rust) | 0.1.2 | Apache-2.0 AND MIT | https://github.com/rust-windowing/winit |
| dtoa (Rust) | 1.0.11 | MIT OR Apache-2.0 | https://github.com/dtolnay/dtoa |
| dtoa-short (Rust) | 0.3.5 | MPL-2.0 | https://github.com/upsuper/dtoa-short |
| dunce (Rust) | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | https://gitlab.com/kornelski/dunce |
| dyn-clone (Rust) | 1.0.20 | MIT OR Apache-2.0 | https://github.com/dtolnay/dyn-clone |
| ecdsa (Rust) | 0.17.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/signatures |
| ed25519 (Rust) | 3.0.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/signatures |
| ed25519-dalek (Rust) | 3.0.0 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/ed25519-dalek |
| elliptic-curve (Rust) | 0.14.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/traits |
| embed-resource (Rust) | 3.0.12 | MIT | https://github.com/nabijaczleweli/rust-embed-resource |
| enum_dispatch (Rust) | 0.3.13 | MIT OR Apache-2.0 | https://gitlab.com/antonok/enum_dispatch |
| equivalent (Rust) | 1.0.3 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/equivalent |
| erased-serde (Rust) | 0.4.10 | MIT OR Apache-2.0 | https://github.com/dtolnay/erased-serde |
| fallible-iterator (Rust) | 0.3.0 | MIT/Apache-2.0 | https://github.com/sfackler/rust-fallible-iterator |
| fallible-streaming-iterator (Rust) | 0.1.9 | MIT/Apache-2.0 | https://github.com/sfackler/fallible-streaming-iterator |
| fastrand (Rust) | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/fastrand |
| fdeflate (Rust) | 0.3.7 | MIT OR Apache-2.0 | https://github.com/image-rs/fdeflate |
| ff (Rust) | 0.14.0 | MIT/Apache-2.0 | https://github.com/zkcrypto/ff |
| filedescriptor (Rust) | 0.8.3 | MIT | https://github.com/wezterm/wezterm |
| find-msvc-tools (Rust) | 0.1.15 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| flate2 (Rust) | 1.1.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/flate2-rs |
| fnv (Rust) | 1.0.7 | Apache-2.0 / MIT | https://github.com/servo/rust-fnv |
| foldhash (Rust) | 0.2.0 | Zlib | https://github.com/orlp/foldhash |
| form_urlencoded (Rust) | 1.2.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| futures (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-channel (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-core (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-executor (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-io (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-macro (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-sink (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-task (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-util (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| generic-array (Rust) | 0.14.7 | MIT | https://github.com/fizyk20/generic-array.git |
| generic-array (Rust) | 1.4.5 | MIT | https://github.com/fizyk20/generic-array.git |
| getrandom (Rust) | 0.2.17 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom (Rust) | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom (Rust) | 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| ghash (Rust) | 0.6.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| glob (Rust) | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-lang/glob |
| group (Rust) | 0.14.0 | MIT/Apache-2.0 | https://github.com/zkcrypto/group |
| h2 (Rust) | 0.4.20 | MIT | https://github.com/hyperium/h2 |
| hash32 (Rust) | 0.3.1 | MIT OR Apache-2.0 | https://github.com/japaric/hash32 |
| hashbrown (Rust) | 0.12.3 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown (Rust) | 0.14.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown (Rust) | 0.17.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashlink (Rust) | 0.12.2 | MIT OR Apache-2.0 | https://github.com/djc/hashlink |
| heapless (Rust) | 0.9.3 | MIT OR Apache-2.0 | https://github.com/rust-embedded/heapless |
| heck (Rust) | 0.5.0 | MIT OR Apache-2.0 | https://github.com/withoutboats/heck |
| hex (Rust) | 0.4.3 | MIT OR Apache-2.0 | https://github.com/KokaKiwi/rust-hex |
| hex-literal (Rust) | 1.1.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| hkdf (Rust) | 0.13.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/KDFs/ |
| hmac (Rust) | 0.12.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/MACs |
| hmac (Rust) | 0.13.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/MACs |
| html5ever (Rust) | 0.39.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| http (Rust) | 1.5.0 | MIT OR Apache-2.0 | https://github.com/hyperium/http |
| http-body (Rust) | 1.1.0 | MIT | https://github.com/hyperium/http-body |
| http-body-util (Rust) | 0.1.5 | MIT | https://github.com/hyperium/http-body |
| httparse (Rust) | 1.10.1 | MIT OR Apache-2.0 | https://github.com/seanmonstar/httparse |
| hybrid-array (Rust) | 0.4.15 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hybrid-array |
| hyper (Rust) | 1.12.0 | MIT | https://github.com/hyperium/hyper |
| hyper-rustls (Rust) | 0.27.10 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/hyper-rustls |
| hyper-tls (Rust) | 0.6.0 | MIT/Apache-2.0 | https://github.com/hyperium/hyper-tls |
| hyper-util (Rust) | 0.1.21 | MIT | https://github.com/hyperium/hyper-util |
| ico (Rust) | 0.6.0 | MIT | https://github.com/mdsteele/rust-ico |
| icu_collections (Rust) | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_locale_core (Rust) | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer (Rust) | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer_data (Rust) | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties (Rust) | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties_data (Rust) | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_provider (Rust) | 2.3.1 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| ident_case (Rust) | 1.0.1 | MIT/Apache-2.0 | https://github.com/TedDriggs/ident_case |
| idna (Rust) | 1.1.0 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| idna_adapter (Rust) | 1.2.2 | Apache-2.0 OR MIT | https://github.com/hsivonen/idna_adapter |
| indexmap (Rust) | 1.9.3 | Apache-2.0 OR MIT | https://github.com/bluss/indexmap |
| indexmap (Rust) | 2.14.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap |
| infer (Rust) | 0.22.0 | MIT | https://github.com/bojand/infer |
| inout (Rust) | 0.1.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| inout (Rust) | 0.2.5 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| ip_network (Rust) | 0.4.1 | BSD-2-Clause | https://github.com/JakubOnderka/ip_network |
| ip_network_table (Rust) | 0.2.0 | BSD-2-Clause | https://github.com/JakubOnderka/ip_network_table |
| ip_network_table-deps-treebitmap (Rust) | 0.5.0 | MIT | https://github.com/JakubOnderka/treebitmap |
| ipnet (Rust) | 2.12.2 | MIT OR Apache-2.0 | https://github.com/krisprice/ipnet |
| itoa (Rust) | 1.0.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/itoa |
| jiff (Rust) | 0.2.38 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-core (Rust) | 0.1.1 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-tzdb (Rust) | 0.1.9 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-tzdb-platform (Rust) | 0.1.3 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| json-patch (Rust) | 4.2.0 | MIT/Apache-2.0 | https://github.com/idubrov/json-patch |
| jsonptr (Rust) | 0.7.1 | MIT OR Apache-2.0 | https://github.com/chanced/jsonptr |
| keccak (Rust) | 0.2.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/sponges |
| kem (Rust) | 0.3.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/traits |
| keyboard-types (Rust) | 0.8.3 | MIT OR Apache-2.0 | https://github.com/rust-windowing/keyboard-types |
| lazy_static (Rust) | 1.5.1 | MIT OR Apache-2.0 | https://github.com/rust-lang-nursery/lazy-static.rs |
| lazy-regex (Rust) | 3.6.2 | MIT | https://github.com/Canop/lazy-regex |
| lazy-regex-proc_macros (Rust) | 3.6.2 | MIT | https://github.com/Canop/lazy-regex/tree/main/src/proc_macros |
| libc (Rust) | 0.2.191 | MIT OR Apache-2.0 | https://github.com/rust-lang/libc |
| libm (Rust) | 0.2.16 | MIT | https://github.com/rust-lang/compiler-builtins |
| libsqlite3-sys (Rust) | 0.38.2 | MIT | https://github.com/rusqlite/rusqlite |
| litemap (Rust) | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| lock_api (Rust) | 0.4.14 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| lock_api (Rust) | 0.5.0 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| log (Rust) | 0.4.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/log |
| managed (Rust) | 0.8.0 | 0BSD | https://github.com/m-labs/rust-managed.git |
| markup5ever (Rust) | 0.39.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| md-5 (Rust) | 0.11.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| md5 (Rust) | 0.8.1 | Apache-2.0 OR MIT | https://github.com/stainless-steel/md5 |
| memchr (Rust) | 2.8.3 | Unlicense OR MIT | https://github.com/BurntSushi/memchr |
| mime (Rust) | 0.3.17 | MIT OR Apache-2.0 | https://github.com/hyperium/mime |
| minisign-verify (Rust) | 0.3.0 | MIT | https://github.com/jedisct1/rust-minisign-verify |
| miniz_oxide (Rust) | 0.8.9 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| miniz_oxide (Rust) | 0.9.1 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| mio (Rust) | 1.2.4 | MIT | https://github.com/tokio-rs/mio |
| ml-kem (Rust) | 0.3.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/KEMs |
| module-lattice (Rust) | 0.2.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/KEMs |
| muda (Rust) | 0.20.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/muda |
| native-tls (Rust) | 0.2.18 | MIT OR Apache-2.0 | https://github.com/rust-native-tls/rust-native-tls |
| netstat2 (Rust) | 0.11.2 | MIT OR Apache-2.0 | https://github.com/ohadravid/netstat2-rs |
| new_debug_unreachable (Rust) | 1.0.6 | MIT | https://github.com/mbrubeck/rust-debug-unreachable |
| nix (Rust) | 0.28.0 | MIT | https://github.com/nix-rust/nix |
| ntapi (Rust) | 0.4.3 | Apache-2.0 OR MIT | https://github.com/MSxDOS/ntapi |
| num-bigint (Rust) | 0.5.1 | MIT OR Apache-2.0 | https://github.com/rust-num/num-bigint |
| num-bigint-dig (Rust) | 0.8.6 | MIT/Apache-2.0 | https://github.com/dignifiedquire/num-bigint |
| num-conv (Rust) | 0.2.2 | MIT OR Apache-2.0 | https://github.com/jhpratt/num-conv |
| num-integer (Rust) | 0.1.47 | MIT OR Apache-2.0 | https://github.com/rust-num/num-integer |
| num-iter (Rust) | 0.1.46 | MIT OR Apache-2.0 | https://github.com/rust-num/num-iter |
| num-traits (Rust) | 0.2.19 | MIT OR Apache-2.0 | https://github.com/rust-num/num-traits |
| once_cell (Rust) | 1.21.4 | MIT OR Apache-2.0 | https://github.com/matklad/once_cell |
| opaque-debug (Rust) | 0.3.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| open (Rust) | 5.4.4 | MIT | https://github.com/Byron/open-rs |
| option-ext (Rust) | 0.2.0 | MPL-2.0 | https://github.com/soc/option-ext.git |
| p256 (Rust) | 0.14.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/elliptic-curves |
| p384 (Rust) | 0.14.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/elliptic-curves |
| p521 (Rust) | 0.14.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/elliptic-curves |
| pageant (Rust) | 0.2.4 | Apache-2.0 | https://github.com/warp-tech/russh |
| parking_lot (Rust) | 0.12.5 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| parking_lot (Rust) | 0.13.0 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| parking_lot_core (Rust) | 0.10.0 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| parking_lot_core (Rust) | 0.9.12 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| password-hash (Rust) | 0.6.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| pbkdf2 (Rust) | 0.13.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/password-hashes |
| pem-rfc7468 (Rust) | 0.7.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/pem-rfc7468 |
| pem-rfc7468 (Rust) | 1.0.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| percent-encoding (Rust) | 2.3.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| phc (Rust) | 0.6.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| phf (Rust) | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_codegen (Rust) | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_generator (Rust) | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_macros (Rust) | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_shared (Rust) | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| pin-project (Rust) | 1.1.13 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project |
| pin-project-internal (Rust) | 1.1.13 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project |
| pin-project-lite (Rust) | 0.2.17 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project-lite |
| pkcs1 (Rust) | 0.7.5 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/pkcs1 |
| pkcs1 (Rust) | 0.8.0-rc.4 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| pkcs5 (Rust) | 0.8.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| pkcs8 (Rust) | 0.10.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/pkcs8 |
| pkcs8 (Rust) | 0.11.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| pkg-config (Rust) | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/pkg-config-rs |
| plist (Rust) | 1.10.3 | MIT | https://github.com/ebarnard/rust-plist/ |
| png (Rust) | 0.18.1 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| poly1305 (Rust) | 0.8.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| poly1305 (Rust) | 0.9.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| polyval (Rust) | 0.7.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| portable-atomic (Rust) | 1.15.0 | Apache-2.0 OR MIT | https://github.com/taiki-e/portable-atomic |
| portable-pty (Rust) | 0.9.0 | MIT | https://github.com/wezterm/wezterm |
| potential_utf (Rust) | 0.1.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| powerfmt (Rust) | 0.2.1 | MIT OR Apache-2.0 | https://github.com/jhpratt/powerfmt |
| ppv-lite86 (Rust) | 0.2.21 | MIT OR Apache-2.0 | https://github.com/cryptocorrosion/cryptocorrosion |
| precomputed-hash (Rust) | 0.1.1 | MIT | https://github.com/emilio/precomputed-hash |
| primefield (Rust) | 0.14.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/elliptic-curves |
| primeorder (Rust) | 0.14.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/elliptic-curves |
| proc-macro2 (Rust) | 1.0.107 | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 |
| quick-xml (Rust) | 0.42.0 | MIT | https://github.com/tafia/quick-xml |
| quote (Rust) | 1.0.47 | MIT OR Apache-2.0 | https://github.com/dtolnay/quote |
| rand (Rust) | 0.10.3 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand (Rust) | 0.8.8 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand_chacha (Rust) | 0.3.1 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand_core (Rust) | 0.10.1 | MIT OR Apache-2.0 | https://github.com/rust-random/rand_core |
| rand_core (Rust) | 0.6.4 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| raw-window-handle (Rust) | 0.6.2 | MIT OR Apache-2.0 OR Zlib | https://github.com/rust-windowing/raw-window-handle |
| ref-cast (Rust) | 1.0.27 | MIT OR Apache-2.0 | https://github.com/dtolnay/ref-cast |
| ref-cast-impl (Rust) | 1.0.27 | MIT OR Apache-2.0 | https://github.com/dtolnay/ref-cast |
| regex (Rust) | 1.13.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-automata (Rust) | 0.4.18 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-syntax (Rust) | 0.8.11 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| reqwest (Rust) | 0.13.5 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| rfc6979 (Rust) | 0.6.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/signatures |
| rfd (Rust) | 0.16.0 | MIT | https://github.com/PolyMeilex/rfd |
| ring (Rust) | 0.17.14 | Apache-2.0 AND ISC | https://github.com/briansmith/ring |
| rsa (Rust) | 0.10.0-rc.18 | MIT OR Apache-2.0 | https://github.com/RustCrypto/RSA |
| rsa (Rust) | 0.9.10 | MIT OR Apache-2.0 | https://github.com/RustCrypto/RSA |
| rusqlite (Rust) | 0.40.2 | MIT | https://github.com/rusqlite/rusqlite |
| russh (Rust) | 0.64.1 | Apache-2.0 | https://github.com/warp-tech/russh |
| russh-cryptovec (Rust) | 0.62.0 | Apache-2.0 | https://github.com/warp-tech/russh |
| russh-sftp (Rust) | 3.0.1 | Apache-2.0 | https://github.com/AspectUnk/russh-sftp |
| russh-util (Rust) | 0.52.0 | Apache-2.0 | https://github.com/warp-tech/russh |
| rustc_version (Rust) | 0.4.1 | MIT OR Apache-2.0 | https://github.com/djc/rustc-version-rs |
| rustc-hash (Rust) | 2.1.3 | Apache-2.0 OR MIT | https://github.com/rust-lang/rustc-hash |
| rustls (Rust) | 0.23.46 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/rustls |
| rustls-pki-types (Rust) | 1.15.1 | MIT OR Apache-2.0 | https://github.com/rustls/pki-types |
| rustls-platform-verifier (Rust) | 0.7.1 | MIT OR Apache-2.0 | https://github.com/rustls/rustls-platform-verifier |
| rustls-webpki (Rust) | 0.103.16 | ISC | https://github.com/rustls/webpki |
| rustversion (Rust) | 1.0.23 | MIT OR Apache-2.0 | https://github.com/dtolnay/rustversion |
| ryu (Rust) | 1.0.23 | Apache-2.0 OR BSL-1.0 | https://github.com/dtolnay/ryu |
| salsa20 (Rust) | 0.11.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/stream-ciphers |
| same-file (Rust) | 1.0.6 | Unlicense/MIT | https://github.com/BurntSushi/same-file |
| schannel (Rust) | 0.1.30 | MIT | https://github.com/steffengy/schannel-rs |
| schemars (Rust) | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| schemars (Rust) | 0.9.0 | MIT | https://github.com/GREsau/schemars |
| schemars (Rust) | 1.2.3 | MIT | https://github.com/GREsau/schemars |
| schemars_derive (Rust) | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| scopeguard (Rust) | 1.2.0 | MIT OR Apache-2.0 | https://github.com/bluss/scopeguard |
| scrypt (Rust) | 0.12.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/password-hashes |
| sec1 (Rust) | 0.8.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| selectors (Rust) | 0.38.0 | MPL-2.0 | https://github.com/servo/stylo |
| semver (Rust) | 1.0.28 | MIT OR Apache-2.0 | https://github.com/dtolnay/semver |
| serde (Rust) | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_bytes (Rust) | 0.11.19 | MIT OR Apache-2.0 | https://github.com/serde-rs/bytes |
| serde_core (Rust) | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive (Rust) | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive_internals (Rust) | 0.29.1 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_json (Rust) | 1.0.154 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| serde_repr (Rust) | 0.1.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-repr |
| serde_spanned (Rust) | 1.1.2 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| serde_urlencoded (Rust) | 0.7.1 | MIT/Apache-2.0 | https://github.com/nox/serde_urlencoded |
| serde_with (Rust) | 3.24.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| serde_with_macros (Rust) | 3.24.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| serde-untagged (Rust) | 0.1.9 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-untagged |
| serdect (Rust) | 0.4.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| serial2 (Rust) | 0.2.38 | BSD-2-Clause OR Apache-2.0 | https://github.com/de-vri-es/serial2-rs |
| serialize-to-javascript (Rust) | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| serialize-to-javascript-impl (Rust) | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| serialport (Rust) | 4.10.1 | MPL-2.0 | https://github.com/serialport/serialport-rs |
| servo_arc (Rust) | 0.4.3 | MIT OR Apache-2.0 | https://github.com/servo/stylo |
| sha1 (Rust) | 0.10.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| sha1 (Rust) | 0.11.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| sha2 (Rust) | 0.10.9 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| sha2 (Rust) | 0.11.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| sha3 (Rust) | 0.12.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| shake (Rust) | 0.1.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/XOFs |
| shared_library (Rust) | 0.1.9 | Apache-2.0/MIT | https://github.com/tomaka/shared_library/ |
| shell-words (Rust) | 1.1.1 | MIT/Apache-2.0 | https://github.com/tmiasko/shell-words |
| shlex (Rust) | 2.0.1 | MIT OR Apache-2.0 | https://github.com/comex/rust-shlex |
| signature (Rust) | 2.2.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/traits/tree/master/signature |
| signature (Rust) | 3.0.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/traits |
| simd-adler32 (Rust) | 0.3.10 | MIT | https://github.com/mcountryman/simd-adler32 |
| siphasher (Rust) | 1.0.4 | MIT OR Apache-2.0 | https://github.com/jedisct1/rust-siphash |
| slab (Rust) | 0.4.12 | MIT | https://github.com/tokio-rs/slab |
| smallvec (Rust) | 1.16.3 | MIT OR Apache-2.0 | https://github.com/servo/rust-smallvec |
| smoltcp (Rust) | 0.14.0 | 0BSD | https://github.com/smoltcp-rs/smoltcp.git |
| socket2 (Rust) | 0.6.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/socket2 |
| softbuffer (Rust) | 0.4.8 | MIT OR Apache-2.0 | https://github.com/rust-windowing/softbuffer |
| spin (Rust) | 0.9.9 | MIT | https://github.com/mvdnes/spin-rs.git |
| spki (Rust) | 0.7.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/spki |
| spki (Rust) | 0.8.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| sponge-cursor (Rust) | 0.1.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| ssh-cipher (Rust) | 0.3.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/SSH |
| ssh-encoding (Rust) | 0.3.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/SSH |
| ssh-key (Rust) | 0.7.0-rc.11 | Apache-2.0 OR MIT | https://github.com/RustCrypto/SSH |
| stable_deref_trait (Rust) | 1.2.1 | MIT OR Apache-2.0 | https://github.com/storyyeller/stable_deref_trait |
| static_vcruntime (Rust) | 3.0.0 | MIT OR Apache-2.0 OR Zlib | https://github.com/ChrisDenton/static_vcruntime |
| string_cache (Rust) | 0.9.0 | MIT OR Apache-2.0 | https://github.com/servo/string-cache |
| string_cache_codegen (Rust) | 0.6.1 | MIT OR Apache-2.0 | https://github.com/servo/string-cache |
| strsim (Rust) | 0.11.1 | MIT | https://github.com/rapidfuzz/strsim-rs |
| subtle (Rust) | 2.6.1 | BSD-3-Clause | https://github.com/dalek-cryptography/subtle |
| suppaftp (Rust) | 12.2.1 | MIT OR Apache-2.0 | https://github.com/veeso/suppaftp |
| syn (Rust) | 2.0.119 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| syn (Rust) | 3.0.9 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| sync_wrapper (Rust) | 1.0.2 | Apache-2.0 | https://github.com/Actyx/sync_wrapper |
| synstructure (Rust) | 0.14.0 | MIT | https://github.com/mystor/synstructure |
| sysinfo (Rust) | 0.39.6 | MIT | https://github.com/GuillaumeGomez/sysinfo |
| tao (Rust) | 0.37.1 | Apache-2.0 | https://github.com/tauri-apps/tao |
| tauri (Rust) | 2.12.5 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-build (Rust) | 2.7.4 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-codegen (Rust) | 2.7.4 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-macros (Rust) | 2.7.4 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-plugin (Rust) | 2.7.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-plugin-dialog (Rust) | 2.8.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-fs (Rust) | 2.7.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-opener (Rust) | 2.7.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-updater (Rust) | 2.14.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-runtime (Rust) | 2.12.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-runtime-wry (Rust) | 2.12.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-utils (Rust) | 2.10.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-winres (Rust) | 0.3.7 | MIT | https://github.com/tauri-apps/winres |
| tempfile (Rust) | 3.27.0 | MIT OR Apache-2.0 | https://github.com/Stebalien/tempfile |
| tendril (Rust) | 0.5.1 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| thiserror (Rust) | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror (Rust) | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror-impl (Rust) | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror-impl (Rust) | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| time (Rust) | 0.3.55 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| time-core (Rust) | 0.1.9 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| time-macros (Rust) | 0.2.32 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| tinystr (Rust) | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| tinyvec (Rust) | 1.13.3 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/tinyvec |
| tokio (Rust) | 1.54.0 | MIT | https://github.com/tokio-rs/tokio |
| tokio-macros (Rust) | 2.7.2 | MIT | https://github.com/tokio-rs/tokio |
| tokio-native-tls (Rust) | 0.3.1 | MIT | https://github.com/tokio-rs/tls |
| tokio-rustls (Rust) | 0.26.6 | MIT OR Apache-2.0 | https://github.com/rustls/tokio-rustls |
| tokio-stream (Rust) | 0.1.19 | MIT | https://github.com/tokio-rs/tokio |
| tokio-tungstenite (Rust) | 0.30.0 | MIT | https://github.com/snapview/tokio-tungstenite |
| tokio-util (Rust) | 0.7.20 | MIT | https://github.com/tokio-rs/tokio |
| toml (Rust) | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_datetime (Rust) | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_parser (Rust) | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_writer (Rust) | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| tower (Rust) | 0.5.3 | MIT | https://github.com/tower-rs/tower |
| tower-http (Rust) | 0.6.11 | MIT | https://github.com/tower-rs/tower-http |
| tower-layer (Rust) | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| tower-service (Rust) | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| tracing (Rust) | 0.1.44 | MIT | https://github.com/tokio-rs/tracing |
| tracing-attributes (Rust) | 0.1.31 | MIT | https://github.com/tokio-rs/tracing |
| tracing-core (Rust) | 0.1.36 | MIT | https://github.com/tokio-rs/tracing |
| tray-icon (Rust) | 0.25.1 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tray-icon |
| try-lock (Rust) | 0.2.5 | MIT | https://github.com/seanmonstar/try-lock |
| tungstenite (Rust) | 0.30.0 | MIT OR Apache-2.0 | https://github.com/snapview/tungstenite-rs |
| typeid (Rust) | 1.0.3 | MIT OR Apache-2.0 | https://github.com/dtolnay/typeid |
| typenum (Rust) | 1.20.1 | MIT OR Apache-2.0 | https://github.com/paholg/typenum |
| unicode-ident (Rust) | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | https://github.com/dtolnay/unicode-ident |
| unicode-segmentation (Rust) | 1.13.3 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-segmentation |
| universal-hash (Rust) | 0.5.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| universal-hash (Rust) | 0.6.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| untrusted (Rust) | 0.9.0 | ISC | https://github.com/briansmith/untrusted |
| url (Rust) | 2.5.8 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| urlpattern (Rust) | 0.6.0 | MIT | https://github.com/denoland/rust-urlpattern |
| utf8_iter (Rust) | 1.0.4 | Apache-2.0 OR MIT | https://github.com/hsivonen/utf8_iter |
| uuid (Rust) | 1.28.0 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| vcpkg (Rust) | 0.2.15 | MIT/Apache-2.0 | https://github.com/mcgoo/vcpkg-rs |
| version_check (Rust) | 0.9.5 | MIT/Apache-2.0 | https://github.com/SergioBenitez/version_check |
| vnc-rs (Rust) | 0.6.0 | MIT OR Apache-2.0 | https://github.com/HsuJv/vnc-rs |
| vswhom (Rust) | 0.1.0 | MIT | https://github.com/nabijaczleweli/vswhom.rs |
| vswhom-sys (Rust) | 0.1.3 | MIT | https://github.com/nabijaczleweli/vswhom-sys.rs |
| walkdir (Rust) | 2.5.0 | Unlicense/MIT | https://github.com/BurntSushi/walkdir |
| want (Rust) | 0.3.2 | MIT | https://github.com/seanmonstar/want |
| web_atoms (Rust) | 0.2.6 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| web-time (Rust) | 1.1.0 | MIT OR Apache-2.0 | https://github.com/daxpedda/web-time |
| webpki-roots (Rust) | 1.0.10 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots |
| webview2-com (Rust) | 0.39.1 | MIT | https://github.com/wravery/webview2-rs |
| webview2-com-macros (Rust) | 0.8.1 | MIT | https://github.com/wravery/webview2-rs |
| webview2-com-sys (Rust) | 0.39.1 | MIT | https://github.com/wravery/webview2-rs |
| winapi (Rust) | 0.3.9 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| winapi-util (Rust) | 0.1.11 | Unlicense OR MIT | https://github.com/BurntSushi/winapi-util |
| window-vibrancy (Rust) | 0.8.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri-plugin-vibrancy |
| windows (Rust) | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc (Rust) | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc (Rust) | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-collections (Rust) | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-core (Rust) | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-future (Rust) | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-implement (Rust) | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-interface (Rust) | 0.59.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-link (Rust) | 0.100.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-link (Rust) | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-numerics (Rust) | 0.3.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-registry (Rust) | 0.100.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-registry (Rust) | 0.6.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result (Rust) | 0.100.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result (Rust) | 0.4.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-strings (Rust) | 0.100.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-strings (Rust) | 0.5.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys (Rust) | 0.52.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys (Rust) | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys (Rust) | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets (Rust) | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets (Rust) | 0.53.5 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-threading (Rust) | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-version (Rust) | 0.1.7 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| winnow (Rust) | 1.0.4 | MIT | https://github.com/winnow-rs/winnow |
| winreg (Rust) | 0.10.1 | MIT | https://github.com/gentoo90/winreg-rs |
| winreg (Rust) | 0.56.0 | MIT | https://github.com/gentoo90/winreg-rs |
| wnaf (Rust) | 0.14.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/elliptic-curves |
| writeable (Rust) | 0.6.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| wry (Rust) | 0.57.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/wry |
| x25519-dalek (Rust) | 2.0.1 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/x25519-dalek |
| yoke (Rust) | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| yoke-derive (Rust) | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerocopy (Rust) | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| zerofrom (Rust) | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerofrom-derive (Rust) | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zeroize (Rust) | 1.9.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| zeroize_derive (Rust) | 1.5.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| zerotrie (Rust) | 0.2.5 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec (Rust) | 0.11.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec-derive (Rust) | 0.11.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zip (Rust) | 4.6.1 | MIT | https://github.com/zip-rs/zip2.git |
| zlib-rs (Rust) | 0.6.8 | Zlib | https://github.com/trifectatechfoundation/zlib-rs |
| zmij (Rust) | 1.0.23 | MIT | https://github.com/dtolnay/zmij |
