# Index every not-yet-indexed major version. Pass extra flags after `--`,
# e.g. `just index -- --engine gecko --limit 5`.
index *args:
    cd indexer && cargo run --release -- index {{args}}

# Index exactly one version, e.g. `just index-one gecko 133` or
# `just index-one webref current`.
index-one engine version:
    cd indexer && cargo run --release -- index-one {{engine}} {{version}}

# Print discovered versions and the source tag each maps to.
list-versions engine:
    cd indexer && cargo run --release -- list-versions {{engine}}

# Re-hash every object and confirm every snapshot entry resolves.
verify:
    cd indexer && cargo run --release -- verify

# Export a fully-resolved snapshot (or, with several `-- --input e:v`,
# their merged common subset) to JSON. E.g. `just export blink 145` or
# `just export -- --input blink:145 --input gecko:140 --out /tmp/merged.json`.
export *args:
    cd indexer && cargo run --release -- export {{args}}

# Convert an exported JSON snapshot into a canonical WIT file. E.g.
# `just canonwit /tmp/snap.json -o /tmp/web.wit`.
canonwit *args:
    cargo run -p canonwit --release -- {{args}}

# Run the frontend dev server.
web:
    cd web && npm run dev

# Build the static frontend bundle (output in web/dist).
build:
    cd web && npm run build
