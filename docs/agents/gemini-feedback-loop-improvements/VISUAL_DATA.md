# Visual Data for Agents

## Philosophy
AI agents effectively "see" code and text better than pixels. While computer vision (multimodal) is possible, it is slow and token-expensive compared to structured semantic data.

## Strategy 1: The "Semantic Viewport" (Recommended)

Instead of a PNG, the game exports a JSON or ASCII representation of the visual field.

### Data Structure (JSON)
```json
{
  "viewport": { "x": 0, "y": 0, "width": 960, "height": 540 },
  "entities": [
    { "id": "player", "x": 0, "y": 0, "sprite": "amy_idle", "layer": 1 },
    { "id": "npc_boba", "x": 32, "y": 0, "sprite": "boba_walk", "layer": 1 },
    { "id": "tree_1", "x": 64, "y": 64, "sprite": "tree_oak", "layer": 2 }
  ]
}
```

### Visualization (ASCII Log)
For quick debugging in logs:
```text
LOG: Visual State
+----------------+
|      T         |
|  P   N         |
|                |
+----------------+
Legend: P=Player, N=NPC, T=Tree
```

## Strategy 2: BRP Scene Query

Use Bevy Remote Protocol to ask "What is visible?"

**Request:**
```json
{
  "method": "game/query_visible_entities",
  "params": { "camera": "MainCamera" }
}
```

**Response:** (List of entities with transforms and bounding boxes)

## Strategy 3: True Headless Rendering

If pixel-perfect verification is required (e.g., checking if a sprite is blurry or if a shader is broken):

1.  **Render-to-Texture:** Create a specialized Bevy plugin that adds a secondary camera rendering to a `Image` asset.
2.  **Extract Buffer:** System reads the `Image` data from the GPU.
3.  **Save:** Write bytes to `capture.png`.

**Note:** This requires a GPU or software rasterizer (Mesa llvmpipe) and cannot run in pure "server" headless mode without `bevy_render`.

## Recommendation for Gemini

Start with **Strategy 1 (Semantic Viewport)** via OTLP logs. It verifies:
1.  Entities are spawned.
2.  Positions are correct.
3.  Game logic considers them "visible".

Implement **Strategy 2** when interaction is needed ("Click the potion").

Defer **Strategy 3** until visual regression testing is strictly required.
