# TileMap Checklist

Automated checks to run after implementation. Each check maps to a gotcha.

## Static Checks

### S1. Collision polygon sizing → G1
Inspect TileSet collision polygons:
- Flag polygons whose extents match tile_size exactly
- Expected: polygons 1-2 px smaller per side than tile dimensions

### S2. TileData direct mutation → G3
Grep for `get_cell_tile_data` followed by `set_custom_data` or property assignment:
- Flag any mutation on the returned TileData reference
- Expected: `_tile_data_runtime_update` virtual or external Dictionary

### S3. Physics layer on wrong node → G5
Grep for `collision_layer` or `collision_mask` assignment on TileMapLayer nodes:
- Flag `collision_layer`/`collision_mask` set on TileMapLayer
- Expected: configured on TileSet physics layer properties

### S4. Y-sort layer scope → G4
Grep `.tscn` and scripts for `y_sort_enabled = true` on TileMapLayer:
- Flag if applied to ground/wall layers (only decoration/entity-interleave layers should use it)
- Y-sorted TileMapLayer and entities must share a common y-sorted ancestor node
- Flag y-sorted TileMapLayer with large tile counts (performance risk from quadrant-per-row)

### S5. Navigation hierarchy → G7
For TileMapLayers with navigation polygons:
- Flag TileMapLayer not parented under NavigationRegion2D
- Expected: TileMapLayer is child of NavigationRegion2D for baking

## Runtime Checks

### R1. Corridor traversal → G1
1-tile corridor with CharacterBody2D moving through:
```
assert(body.global_position == end_position, "stuck on tile corner — collision polygon too large")
```

### R2. Terrain batch consistency → G2
Paint L-shaped region via `set_cells_terrain_connect` vs cell-by-cell:
```
assert(batch_tile_ids == cellbycell_tile_ids, "terrain solver produced inconsistent results")
```

### R3. TileData isolation → G3
Place 2 cells with same tile, override custom data on cell A via `_tile_data_runtime_update`:
```
assert(cell_b_data == original_value, "TileData shared reference leaked to other cell")
```

### R4. Scene tile erase cleanup → G6
Place scene-collection tile, `erase_cell()`, wait one frame:
```
assert(layer.get_children().size() == 0, "orphaned child nodes remain after scene tile erase")
```

### R5. Nav rebake after tile change → G7
Bake with wall blocking path; erase wall, await physics_frame, rebake:
```
assert(nav_path.size() > 0, "navigation not updated after tile change and rebake")
```

## Compilation

```bash
{godot_path} --headless --quit 2>&1
```
Pass: exit code 0, no TileMap/TileSet-related errors in output.
