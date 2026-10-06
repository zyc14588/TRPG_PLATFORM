// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package profile

import (
	"weak"

	lua "github.com/iceisfun/golua/v2/vm"
)

// checkpointShape retains only the explicit data shape that an ordinary Lua
// table cannot encode. Tables remain the backend's ordinary concrete tables:
// indexing, iteration, length, nil and metatables keep their Lua semantics.
// No sentinel, metatable behavior, userdata or host handle enters the script.
type checkpointShape struct {
	array   bool
	nilKeys map[lua.Value]bool
}

func (e *Engine) checkpointTable(array bool) (*lua.Table, *checkpointShape) {
	table := lua.NewEmptyTable()
	shape := &checkpointShape{array: array, nilKeys: map[lua.Value]bool{}}
	e.shapes[weak.Make(table)] = shape
	return table, shape
}

// Observe the completed invocation before converting its results. A nil entry
// observed with a non-nil value loses its old presence marker, so deletion in a
// later invocation cannot resurrect that marker.
// Weak keys avoid retaining tables that Lua has replaced or discarded. Metadata
// is per engine, derived only from explicit state/checkpoints and cleared on close.
func (e *Engine) reconcileCheckpointShapes() {
	for ref, shape := range e.shapes {
		table := ref.Value()
		if table == nil {
			delete(e.shapes, ref)
			continue
		}
		for key := range shape.nilKeys {
			if !table.Get(key).IsNil() {
				delete(shape.nilKeys, key)
			}
		}
		if shape.array {
			key := lua.Nil
			for {
				next, _, err := table.Next(key)
				if err != nil || next.IsNil() {
					break
				}
				if !next.IsInt() || next.AsInt() < 1 {
					shape.array = false
					break
				}
				key = next
			}
		}
	}
}
