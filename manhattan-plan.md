# Manhattan Routing Implementation Plan

## Current State
- Single center pin per edge (top/right/bottom/left)
- Simple L-shaped routing (horizontal-first)
- 10px rounded bezier corners
- Connections can overlap when multiple lines use same edge

## Phase 1: Port Distribution ✅ 
**Goal**: Eliminate overlapping connection origins/targets

**Implementation**: ✅ COMPLETED
- ✅ Replace single center pin with multiple distributed ports per edge
- ✅ Group connections by source/target edge
- ✅ Sort targets spatially (currently by entity ID, can be improved)
- ✅ Distribute ports evenly along edge length
- ✅ Update `EdgePins` to support `Vec<Pos2>` per edge instead of single `Pos2`

## Phase 2: Enhanced Crossing Avoidance ✅
**Goal**: Smart port assignment with lookahead to minimize crossings

**Implementation**: ✅ COMPLETED (Enhanced Greedy with Lookahead)
- ✅ **Constraint-based prioritization**: Route most constrained connections first
- ✅ **Lookahead scoring**: Consider impact on future connections when choosing ports
- ✅ **Crossing minimization**: Try both horizontal-first and vertical-first routing
- ✅ **Port removal**: Remove used ports from available pools to prevent conflicts
- ✅ **O(n²) complexity**: Much faster than O(n³) optimal algorithms while still being smart

**Algorithm Details**:
- Sort connections by constraint level (fewer available port options = higher priority)
- For each connection, try all available port combinations
- Score each assignment based on: immediate crossings, future impact, distance
- Choose best assignment and remove used ports from future consideration
- Handles diagonal connection cases that were causing suboptimal routing

## Phase 3: S-Shape Connections & Bend Staggering ✅
**Goal**: Advanced routing with perpendicular entry/exit and bend staggering

**Implementation**: ✅ COMPLETED
- ✅ **S-Shape Support**: Added `ConnectionShape` enum with `LShape` and `SShape` options
- ✅ **Perpendicular Entry/Exit**: Connections extend perpendicular from edges before turning
- ✅ **Smart Shape Selection**: Use S-shape for close nodes, same-edge connections, or high-parallel scenarios
- ✅ **Bend Staggering**: Automatic 15px stagger per parallel connection to prevent overlaps
- ✅ **Multi-Segment Routing**: Support for 2+ bend points with smooth rounded corners

## Phase 4: Perpendicular Arrows ✅
**Goal**: Arrow heads point into target faces, not parallel to them

**Implementation**: ✅ COMPLETED
- ✅ **Edge-Based Arrow Direction**: Arrow heads always point perpendicular to target edge
- ✅ **Proper Arrow Orientation**: Top/Right/Bottom/Left edge arrows point in correct directions
- ✅ **Enhanced Drawing System**: Complete rewrite of connection rendering with multi-segment support

## Notes
- Start simple, add complexity incrementally
- Prioritize visual quality over algorithmic perfection
- Users should create reasonable layouts that algorithms can enhance
