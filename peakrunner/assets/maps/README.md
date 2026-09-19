# Original map assets

`raindance/` is generated from `maps/raindance.json` by
`scripts/build-original-map.py`, using only the Python standard library.
All geometry, materials, terrain and ambient audio in this directory are
procedural PeakRunner assets. There are no extracted Tribes assets here.

See `docs/original-map-kit.md` for authoring, gameplay and verification details.
Keep `map.json` and its six binary files together. Native clients, servers and
WebAssembly builds embed the same default content and validate its checksums.
