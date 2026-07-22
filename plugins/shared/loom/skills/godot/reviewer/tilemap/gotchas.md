# TileMap Gotchas

Design-level constraints — intentional engine behaviors, not bugs. Stable across Godot 4.x.

## G1. Tile collision polygon corner snagging [GDScript] [C#]

**Symptom**: Character gets stuck entering 1-tile-wide corridors or catches on wall corners.

**Root cause**: Exact-tile-size collision polygons create micro-ledges at shared edges.

**Correct approach**: Collision polygons 1-2 px smaller per side. Isometric: shrink the diamond inward along all edges.

**Wrong approach**: Exact tile-size polygons; movement-code workarounds instead of fixing polygons; isometric tiles with only rectangular shrink.

## G2. Terrain solver is local and order-dependent [GDScript] [C#]

**Symptom**: Terrain-painted tiles show wrong transitions or gaps depending on paint order.

**Root cause**: Greedy local solver — no backtracking. Each cell resolves based on current neighbor state.

**Correct approach**: Batch paint via `set_cells_terrain_connect`. Ensure tileset has every transition variant.

**Wrong approach**: Cell-by-cell painting expecting global consistency; cross-terrain-set transitions (they don't interact); missing variants expecting solver to improvise.

## G3. TileData is a shared reference [GDScript] [C#]

**Symptom**: Modifying one tile's custom data at runtime changes ALL instances of that tile.

**Root cause**: `get_cell_tile_data()` returns a shared reference — no per-cell copy.

**Correct approach**: `_tile_data_runtime_update` virtual for per-cell overrides (enable only for cells that need it). Or external Dictionary keyed by coords.

**Wrong approach**: `layer.get_cell_tile_data(coords).set_custom_data("hp", 50)` — mutates ALL cells using that tile. `[C#]` same: `GetCellTileData(coords).SetCustomData(...)`.

## G4. Y-sort disables quadrant batching [GDScript] [C#]

**Symptom**: Severe FPS drop on large tilemaps with `y_sort_enabled`.

**Root cause**: Y-sort forces per-row rendering quadrants, defeating draw-call batching.

**Correct approach**: Only `y_sort_enabled` on decoration layers where entities interleave with tiles. Ground/wall layers stay y-sort-free. Y-sorted layer and entities need a common y-sorted ancestor.

**Wrong approach**: `y_sort_enabled` on all layers; y-sorted entities and TileMapLayer under different parents (breaks ordering).

## G5. Physics config lives on TileSet, not TileMapLayer [GDScript] [C#]

**Symptom**: `collision_layer`/`collision_mask` set on TileMapLayer node has no effect on tile collisions.

**Root cause**: Tile collision config belongs to TileSet physics layers. TileMapLayer only has `collision_enabled` toggle.

**Correct approach**: Set `collision_layer`/`collision_mask` on TileSet physics layer properties.

**Wrong approach**: Setting `collision_layer`/`collision_mask` on TileMapLayer node; looking in TileMapLayer inspector instead of TileSet editor.

## G6. Scene tile cleanup on erase [GDScript] [C#]

**Symptom**: Erasing scene-collection tiles leaves orphaned child nodes.

**Root cause**: Scene tiles instantiate full node trees. Erase path doesn't reliably free all instantiated nodes.

**Correct approach**: After `erase_cell()` on scene tiles, verify child cleanup. Manually free orphans if any.

**Wrong approach**: Assuming `erase_cell()` on scene tiles is as clean as atlas tiles; not checking for remaining children.

## G7. Navigation mesh goes stale after tile changes [GDScript] [C#]

**Symptom**: After erasing or placing tiles at runtime, pathfinding agents walk through new walls or avoid removed ones.

**Root cause**: Nav mesh is a baked snapshot — cell changes don't trigger rebake.

**Correct approach**: After tile changes, `await get_tree().physics_frame` then `bake_navigation_polygon()`. `[C#]` `await ToSignal(GetTree(), SceneTree.SignalName.PhysicsFrame)`.

**Wrong approach**: Assuming nav auto-updates; rebaking per-cell in a loop; skipping the physics frame wait before rebake.
