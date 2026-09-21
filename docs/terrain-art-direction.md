# Terrain art direction and Tribes research

## Target

Readable green turf, dry earth, and exposed rock with granular surface detail.
No upright grass blades, camera-following vegetation, directional streaks, or
camera-distance changes to texture coordinates. Keep the collision heightfield
and the playtested movement unchanged. The user-provided Tribes 1 screenshot is
the visual reference, not a requirement to reproduce its exact low resolution.

## What the originals did

- Mark Frohnmayer described Tribes 1's regular 8 m height grid, per-square
  terrain textures, and combined textures for lower detail levels. Tribes 2
  moved to a different screen-error scheme. This is terrain mesh plus surface
  texturing, not a field of individual blades.
  [Developer interview](https://www.gamedeveloper.com/programming/real-time-dynamic-level-of-detail-terrain-rendering-with-roam)
- TribesNEXT renderer work describes terrain material weight maps, a detail
  texture, and an original blender supporting eight materials with four dominant
  weights at a pixel. Use this separation of material placement and detail as a
  design reference; do not reproduce the old per-frame CPU blending pipeline.
  [Renderer implementation notes](https://www.tribesnext.com/forum/discussion/4335/boxedwine-as-a-path-to-running-tribes-2-in-a-web-browser)
- Tribes 1 weapons are textured DTS meshes. The TribesToBlender author documents
  extracting meshes and companion textures from .vol (1.11) or .zip (1.40),
  then importing them together. For the disc launcher, studying actual UVs and
  painted surface details is the next useful reference step beyond silhouette.
  No original weapon assets were extracted or redistributed in this pass.
  [Importer documentation](https://github.com/tekrog/TribesToBlender)
- TribesNEXT provides the maintained community entry point for Tribes 2.
  No installer was executed in this pass.
  [TribesNEXT](https://www.tribesnext.com/)

The supplied YouTube page (nn97u45Nl3E) could not be retrieved. No claim is made
to have watched it or inferred its contents.

## Implemented foundation

1. Height/collision stays in terrain.rs.
2. Each map selects a SurfaceStyle: cover/soil/rock palette, physical repeat
   size, macro patch scale, grain contrast, slope blend, and normal strength.
3. grass.rs now bakes a deterministic neutral, isotropic detail tile and a
   wrapping normal map. The legacy module name is retained to avoid unnecessary
   churn; it no longer contains blades or tuft placement.
4. The existing mipmap/anisotropic filtering path is preserved. Fine grain
   naturally becomes less visible with distance without moving its world scale.
5. Raindance uses the layered material path. Valley retains its established
   snow shading; its style descriptor is ready for later migration, not a claim
   that the snow material has been rebuilt.

## Next landscape pipeline, in order

1. Author three distinct tileable albedo/detail sets per biome with documented
   meters-per-repeat, palette, and contrast targets. Track asset provenance.
2. Add optional painted material-weight maps alongside slope/elevation rules,
   so paths, ridges, wet areas, and base approaches are deliberately placed.
3. Add triplanar rock mapping for steep cliffs; keep grass/soil mapping stable.
4. Group atmosphere, lighting, and fog into map presets separate from materials.
5. Add terrain chunk LOD/culling only after profiling; never silently change the
   rendered/contact surface agreement to gain visual smoothness.

QA views: ground-level, downward, ridge/grazing-angle, moving camera, distant
flyby, and narrow screens. Check for repetition, swimming, shimmer, material
readability, and GPU cost. Still captures alone do not certify temporal quality.
