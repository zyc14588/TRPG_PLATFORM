// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package realtime

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
)

// FilterView owns and validates even direct policy arguments before traversal.
// A malformed policy or any invalid part of the checkpoint denies the full view.
func FilterView(v checkpoint.Value, p command.ViewPolicy) checkpoint.Value {
	out := checkpoint.Object(map[string]checkpoint.Value{})
	owned, err := command.OwnPolicy(p)
	if err != nil || v.Kind != "table" || checkpoint.Validate(v) != nil {
		return out
	}
	// Preserve the legacy scalar selections, including explicitly present nil.
	for _, name := range owned.ViewFields {
		if x, ok := v.Table[name]; ok && x.Kind != "table" && x.Kind != "array" {
			out.Table[name] = x
		}
	}
	if owned.ViewProjection != nil {
		for name, selector := range owned.ViewProjection.Fields {
			if x, exists := v.Table[name]; exists {
				if filtered, ok := projectView(x, selector); ok {
					out.Table[name] = filtered
				}
			}
		}
	}
	return out
}

// The policy and complete checkpoint are already bounded and validated. Each
// selected container is newly allocated; a denied array element denies its array.
func projectView(v checkpoint.Value, s *command.ViewSelector) (checkpoint.Value, bool) {
	if v.Kind == "nil" {
		return checkpoint.Value{Kind: "nil"}, s.Nullable
	}
	switch s.Kind {
	case "scalar":
		switch v.Kind {
		case "string", "integer", "float", "boolean":
			return v, true
		}
	case "table":
		if v.Kind != "table" {
			break
		}
		out := checkpoint.Object(make(map[string]checkpoint.Value, len(s.Fields)))
		for name, child := range s.Fields {
			if x, exists := v.Table[name]; exists {
				if filtered, ok := projectView(x, child); ok {
					out.Table[name] = filtered
				}
			}
		}
		return out, true
	case "array":
		if v.Kind != "array" {
			break
		}
		out := make([]checkpoint.Value, len(v.Array))
		for n, x := range v.Array {
			filtered, ok := projectView(x, s.Element)
			if !ok {
				return checkpoint.Value{}, false
			}
			out[n] = filtered
		}
		return checkpoint.Array(out...), true
	}
	return checkpoint.Value{}, false
}
