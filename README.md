# linux-borderlands-editor

A native Linux save editor for **Borderlands 2** — our own product, not a port.
One binary that is **both** a GUI (Gibbed feature parity) **and** a CLI editor.
No Wine, no Mono, no .NET, no WPF.

**Stack:** Rust. A `bl2-save` core library (the whole save format, item serials,
and game database) with a CLI frontend and a desktop GUI on top. Logic is ported
to Rust from Gibbed's open-source (zlib) tools and cross-checked against
apocalyptech's Python implementation.

## Status
- ✅ **Format round-trip proven** in Rust (`poc-roundtrip/`) against both sample
  saves — LZO1x + custom Huffman + CRC32 + SHA1 decode/re-encode byte-correctly.
- ⏭ **Next:** edit money/eridium, re-encode, and confirm the game loads it — then
  the item/weapon serial editor.

## Build & run (host stays toolchain-free)
Everything builds inside a container — nothing is installed on your machine:
```bash
./run.sh                              # build + run the PoC on save0001.sav
./run.sh run ../samples/save0002.sav  # pass args through to cargo
./run.sh test                         # any cargo subcommand
docker build -t bl2edit .             # reproducible release image
```
For IDE development, open the project in RustRover (it will install a native
rustup toolchain on first open).

## Start here
👉 **Read [`PLAN.md`](./PLAN.md) fully before writing any code** — verified save
format, the proven round-trip, reuse strategy, milestones, and safety rules.

## Layout
- `poc-roundtrip/` — Rust proof-of-concept that proves the format round-trips.
- `Dockerfile`, `run.sh` — containerized build (see above).
- `samples/` — non-destructive **copies** of real `.sav` files to test against.
  Never test against the live saves until in-game load is proven.
- `PLAN.md` — full implementation brief / handoff document.
