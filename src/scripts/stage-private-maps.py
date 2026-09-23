#!/usr/bin/env python3
"""Copy verified runtime files for the three reference maps.

The broadside-clone slot is the original Tower Complex, embedded in the binary.
"""
import hashlib
import json
import pathlib
import shutil
import sys

workspace = pathlib.Path(__file__).resolve().parent.parent
destination = pathlib.Path(sys.argv[1]).resolve()
if destination.exists():
    raise SystemExit("Refusing to overwrite private map stage")
payloads = ("vertices.bin", "collision.bin", "height.bin", "weights.rgba", "textures.rgba", "ambient.f32")
packs = {}
for key in ("stonehenge-clone", "snowblind-clone", "desert-of-death-clone"):
    source = workspace / "local-assets" / key / "installed"
    manifest = json.loads((source / "map.json").read_text())
    if not manifest.get("private_reference"):
        raise SystemExit(f"Expected private map: {key}")
    for name in payloads:
        if hashlib.sha256((source / name).read_bytes()).hexdigest() != manifest["files"][name]:
            raise SystemExit(f"Checksum mismatch: {key}/{name}")
    packs[key] = source
for key, source in packs.items():
    target = destination / key
    target.mkdir(parents=True)
    for name in ("map.json", *payloads):
        shutil.copyfile(source / name, target / name)
print(f"Staged {len(packs)} private maps, seven verified files per map: {destination}")
