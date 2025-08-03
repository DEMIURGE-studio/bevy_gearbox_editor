# Parent-Child Zone Implementation Plan

## Overview
Extend the node editor to support hierarchical state machines by implementing visual "zones" for parent entities, similar to Stately's approach for XState visualization.

## Core Concept
- **Parent entities** (with `Children` component) are rendered as resizable rectangular zones
- **Child entities** are visually contained within these zones
- **Hierarchy management** through drag-and-drop zone interaction
- **Initial state pointers** connect parent zones to their default child state

## Visual Design

### Node Types
- **Leaf nodes**: Header + Transitions (current system)
- **Parent nodes**: Header + Zone area (no transitions section)

### Zone Behavior
- **Manual resize only**: Users drag zone edges to resize
- **Boundary-based ownership**: Entities inside a zone become children via `ChildOf` component
- **Automatic hierarchy updates**: `ChildOf` automatically manages `Children` relationships
- **Nested zones supported**: Entities belong to the deepest (smallest area) containing zone

### Initial State Visualization
- **Position**: Fixed in top-left corner of parent zone
- **Connection**: Bezier line to target child (identical style to transitions)
- **Component**: Represents `bevy_gearbox::InitialState` pointing to default child

## Technical Implementation

### Multi-Pass Rendering Extension
```
PASS 0: Zone backgrounds (deepest first for proper layering)
PASS 1: Input handling (zones, resize handles, nodes)  
PASS 2: Unselected nodes
PASS 3: Connections + Initial state pointers (reuse existing bezier code)
PASS 4: Selected nodes
PASS 5: Zone borders + resize handles (on top)
```

### Data Model Extensions
- `ParentZone` component: stores zone bounds and resize handle areas
- `InitialStatePointer` component: references target child entity
- Zone ownership calculation: find deepest containing zone for position

### Interaction Behaviors
- **Entity drag**: Check final position against all zones for ownership changes
- **Parent drag**: Move parent + all descendants together (coordinated movement)
- **Resize drag**: Update zone bounds, recalculate ownership for all entities
- **Boundary crossing**: Add/remove `ChildOf` components automatically

### Hit-Testing Priority
1. Resize handles (highest priority - thin rectangles along zone edges)
2. Nodes (existing system)
3. Zone interiors (for drop detection)

## Implementation Phases

### Phase 1: Basic Zones
- Extend `NodeRenderer` to handle parent vs leaf node types
- Add zone rectangle rendering
- Implement single-level ownership calculation

### Phase 2: Interaction System
- Zone boundary drag detection and ownership management
- `ChildOf` component addition/removal on boundary crossing
- Parent-child coordinated movement system

### Phase 3: Advanced Features
- Nested zone ownership ("deepest wins" algorithm)  
- Resize handle interaction and edge dragging
- Initial state pointer rendering using existing connection system

## Architecture Benefits
- Leverages existing multi-pass rendering for proper layering
- Reuses connection renderer for initial state pointers
- ECS `Children`/`ChildOf` relationships handle hierarchy automatically
- Widget system easily accommodates different node types
- Existing resource caching (sizes, positions) works for zones

## Design Decisions
- **Manual resize only**: No auto-sizing complexity
- **Clear boundary rules**: In/out semantics for ownership
- **Deepest wins**: Nested zones ownership based on smallest containing area
- **Identical connections**: Initial state pointers use same visual style as transitions
- **Fixed initial state position**: Top-left corner eliminates layout complexity
- **Coordinated movement**: Parent drag moves entire subtree together
