// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package checkpoint

import (
	"errors"
	"fmt"
	"math"
	"reflect"
	"sort"
	"unicode/utf8"

	luavm "github.com/iceisfun/golua/v2/vm"
)

const (
	MaxDepth       = 64
	MaxValueNodes  = 100_000
	MaxStringBytes = 1 << 20
)

var (
	ErrUnsupportedValue = errors.New("checkpoint value type is not supported")
	ErrCycle            = errors.New("checkpoint value contains a cycle")
	ErrAliasedTable     = errors.New("checkpoint value contains an aliased table")
	ErrMetatable        = errors.New("checkpoint table has a metatable")
	ErrInvalidValue     = errors.New("checkpoint value is invalid")
	ErrLimit            = errors.New("checkpoint value exceeds a limit")
)

type ValueType string

const (
	TypeNil    ValueType = "nil"
	TypeBool   ValueType = "boolean"
	TypeInt    ValueType = "integer"
	TypeFloat  ValueType = "float"
	TypeString ValueType = "string"
	TypeArray  ValueType = "array"
	TypeTable  ValueType = "table"
)

// Field is one canonical string-key table entry. Fields are sorted by UTF-8
// byte order when encoded.
type Field struct {
	Key   string `json:"key"`
	Value Value  `json:"value"`
}

// Value is the complete checkpoint value algebra. It deliberately has no
// representation for functions, threads, userdata, handles or metatables.
type Value struct {
	Type    ValueType `json:"type"`
	Boolean bool      `json:"boolean,omitempty"`
	Integer int64     `json:"integer,omitempty"`
	Float   float64   `json:"float,omitempty"`
	String  string    `json:"string,omitempty"`
	Array   []Value   `json:"array,omitempty"`
	Table   []Field   `json:"table,omitempty"`
}

func Nil() Value                    { return Value{Type: TypeNil} }
func Bool(v bool) Value             { return Value{Type: TypeBool, Boolean: v} }
func Int(v int64) Value             { return Value{Type: TypeInt, Integer: v} }
func Float(v float64) Value         { return Value{Type: TypeFloat, Float: v} }
func String(v string) Value         { return Value{Type: TypeString, String: v} }
func Array(v ...Value) Value        { return Value{Type: TypeArray, Array: append([]Value(nil), v...)} }
func Table(v ...Field) Value        { return Value{Type: TypeTable, Table: append([]Field(nil), v...)} }
func Entry(k string, v Value) Field { return Field{Key: k, Value: v} }

type visit struct {
	kind byte
	ptr  uintptr
}

type valueWalker struct {
	active map[visit]bool
	seen   map[visit]bool
	nodes  int
}

func newValueWalker() *valueWalker {
	return &valueWalker{active: make(map[visit]bool), seen: make(map[visit]bool)}
}

func (w *valueWalker) count(depth int) error {
	if depth > MaxDepth {
		return fmt.Errorf("%w: depth exceeds %d", ErrLimit, MaxDepth)
	}
	w.nodes++
	if w.nodes > MaxValueNodes {
		return fmt.Errorf("%w: nodes exceed %d", ErrLimit, MaxValueNodes)
	}
	return nil
}

func (w *valueWalker) enter(v visit) error {
	if v.ptr == 0 {
		return nil
	}
	if w.active[v] {
		return ErrCycle
	}
	if w.seen[v] {
		return ErrAliasedTable
	}
	w.active[v] = true
	w.seen[v] = true
	return nil
}

func (w *valueWalker) leave(v visit) {
	if v.ptr != 0 {
		delete(w.active, v)
	}
}

// Normalize validates a Value and returns its canonical table ordering.
func Normalize(value Value) (Value, error) {
	return newValueWalker().normalize(value, 0)
}

func (w *valueWalker) normalize(value Value, depth int) (Value, error) {
	if err := w.count(depth); err != nil {
		return Value{}, err
	}
	switch value.Type {
	case TypeNil:
		if value.Boolean || value.Integer != 0 || value.Float != 0 || value.String != "" || value.Array != nil || value.Table != nil {
			return Value{}, ErrInvalidValue
		}
		return Nil(), nil
	case TypeBool:
		if value.Integer != 0 || value.Float != 0 || value.String != "" || value.Array != nil || value.Table != nil {
			return Value{}, ErrInvalidValue
		}
		return Bool(value.Boolean), nil
	case TypeInt:
		if value.Boolean || value.Float != 0 || value.String != "" || value.Array != nil || value.Table != nil {
			return Value{}, ErrInvalidValue
		}
		return Int(value.Integer), nil
	case TypeFloat:
		if value.Boolean || value.Integer != 0 || value.String != "" || value.Array != nil || value.Table != nil || math.IsNaN(value.Float) || math.IsInf(value.Float, 0) {
			return Value{}, ErrInvalidValue
		}
		return Float(value.Float), nil
	case TypeString:
		if value.Boolean || value.Integer != 0 || value.Float != 0 || value.Array != nil || value.Table != nil || !utf8.ValidString(value.String) {
			return Value{}, ErrInvalidValue
		}
		if len(value.String) > MaxStringBytes {
			return Value{}, fmt.Errorf("%w: string exceeds %d bytes", ErrLimit, MaxStringBytes)
		}
		return String(value.String), nil
	case TypeArray:
		if value.Boolean || value.Integer != 0 || value.Float != 0 || value.String != "" || value.Table != nil {
			return Value{}, ErrInvalidValue
		}
		marker := visit{kind: 'a', ptr: reflect.ValueOf(value.Array).Pointer()}
		if err := w.enter(marker); err != nil {
			return Value{}, err
		}
		defer w.leave(marker)
		result := Value{Type: TypeArray, Array: make([]Value, len(value.Array))}
		for i := range value.Array {
			item, err := w.normalize(value.Array[i], depth+1)
			if err != nil {
				return Value{}, fmt.Errorf("array[%d]: %w", i, err)
			}
			result.Array[i] = item
		}
		return result, nil
	case TypeTable:
		if value.Boolean || value.Integer != 0 || value.Float != 0 || value.String != "" || value.Array != nil {
			return Value{}, ErrInvalidValue
		}
		marker := visit{kind: 't', ptr: reflect.ValueOf(value.Table).Pointer()}
		if err := w.enter(marker); err != nil {
			return Value{}, err
		}
		defer w.leave(marker)
		result := Value{Type: TypeTable, Table: make([]Field, len(value.Table))}
		keys := make(map[string]struct{}, len(value.Table))
		for i, field := range value.Table {
			if !utf8.ValidString(field.Key) {
				return Value{}, fmt.Errorf("table key %d: %w", i, ErrInvalidValue)
			}
			if len(field.Key) > MaxStringBytes {
				return Value{}, fmt.Errorf("table key %d: %w", i, ErrLimit)
			}
			if _, exists := keys[field.Key]; exists {
				return Value{}, fmt.Errorf("duplicate table key %q: %w", field.Key, ErrInvalidValue)
			}
			keys[field.Key] = struct{}{}
			item, err := w.normalize(field.Value, depth+1)
			if err != nil {
				return Value{}, fmt.Errorf("table[%q]: %w", field.Key, err)
			}
			result.Table[i] = Field{Key: field.Key, Value: item}
		}
		sort.Slice(result.Table, func(i, j int) bool { return result.Table[i].Key < result.Table[j].Key })
		return result, nil
	default:
		return Value{}, fmt.Errorf("%w: type %q", ErrInvalidValue, value.Type)
	}
}

// FromGo converts the intentionally small set of supported Go data shapes.
func FromGo(input any) (Value, error) {
	return newValueWalker().fromGo(input, 0)
}

func (w *valueWalker) fromGo(input any, depth int) (Value, error) {
	if err := w.count(depth); err != nil {
		return Value{}, err
	}
	switch value := input.(type) {
	case nil:
		return Nil(), nil
	case Value:
		// Use a separate walker because this node was already counted here.
		return Normalize(value)
	case bool:
		return Bool(value), nil
	case string:
		if !utf8.ValidString(value) {
			return Value{}, ErrInvalidValue
		}
		if len(value) > MaxStringBytes {
			return Value{}, ErrLimit
		}
		return String(value), nil
	case int:
		return Int(int64(value)), nil
	case int8:
		return Int(int64(value)), nil
	case int16:
		return Int(int64(value)), nil
	case int32:
		return Int(int64(value)), nil
	case int64:
		return Int(value), nil
	case uint:
		if uint64(value) > math.MaxInt64 {
			return Value{}, ErrInvalidValue
		}
		return Int(int64(value)), nil
	case uint8:
		return Int(int64(value)), nil
	case uint16:
		return Int(int64(value)), nil
	case uint32:
		return Int(int64(value)), nil
	case uint64:
		if value > math.MaxInt64 {
			return Value{}, ErrInvalidValue
		}
		return Int(int64(value)), nil
	case float32:
		return finiteFloat(float64(value))
	case float64:
		return finiteFloat(value)
	case []any:
		marker := visit{kind: 'g', ptr: reflect.ValueOf(value).Pointer()}
		if err := w.enter(marker); err != nil {
			return Value{}, err
		}
		defer w.leave(marker)
		out := Value{Type: TypeArray, Array: make([]Value, len(value))}
		for i := range value {
			item, err := w.fromGo(value[i], depth+1)
			if err != nil {
				return Value{}, fmt.Errorf("array[%d]: %w", i, err)
			}
			out.Array[i] = item
		}
		return out, nil
	case map[string]any:
		marker := visit{kind: 'm', ptr: reflect.ValueOf(value).Pointer()}
		if err := w.enter(marker); err != nil {
			return Value{}, err
		}
		defer w.leave(marker)
		out := Value{Type: TypeTable, Table: make([]Field, 0, len(value))}
		for key, raw := range value {
			item, err := w.fromGo(raw, depth+1)
			if err != nil {
				return Value{}, fmt.Errorf("table[%q]: %w", key, err)
			}
			out.Table = append(out.Table, Field{Key: key, Value: item})
		}
		return Normalize(out)
	case luavm.Value:
		// Compensate for the count performed above before delegating.
		w.nodes--
		return w.fromLua(value, depth)
	default:
		return Value{}, fmt.Errorf("%w: %T", ErrUnsupportedValue, input)
	}
}

func finiteFloat(value float64) (Value, error) {
	if math.IsNaN(value) || math.IsInf(value, 0) {
		return Value{}, ErrInvalidValue
	}
	return Float(value), nil
}

// FromLua copies a supported Lua value into checkpoint-safe data.
func FromLua(value luavm.Value) (Value, error) {
	return newValueWalker().fromLua(value, 0)
}

func (w *valueWalker) fromLua(value luavm.Value, depth int) (Value, error) {
	if err := w.count(depth); err != nil {
		return Value{}, err
	}
	switch {
	case value.IsNil():
		return Nil(), nil
	case value.IsBool():
		return Bool(value.AsBool()), nil
	case value.IsInt():
		return Int(value.AsInt()), nil
	case value.IsFloat():
		return finiteFloat(value.AsFloat())
	case value.IsString():
		return w.fromGo(value.AsString(), depth)
	case value.Type() == "thread":
		return Value{}, fmt.Errorf("%w: Lua coroutine", ErrUnsupportedValue)
	case value.IsTable():
		return w.fromLuaTable(value.AsTable(), depth)
	case value.IsFunction(), value.IsNativeFunc():
		return Value{}, fmt.Errorf("%w: Lua function", ErrUnsupportedValue)
	case value.IsUserdata(), value.IsLightUserdata():
		return Value{}, fmt.Errorf("%w: Lua userdata/native handle", ErrUnsupportedValue)
	default:
		return Value{}, fmt.Errorf("%w: Lua %s", ErrUnsupportedValue, value.Type())
	}
}

func (w *valueWalker) fromLuaTable(table luavm.LuaTable, depth int) (Value, error) {
	if table.Metatable() != nil {
		return Value{}, ErrMetatable
	}
	marker := visit{kind: 'l', ptr: reflect.ValueOf(table).Pointer()}
	if err := w.enter(marker); err != nil {
		return Value{}, err
	}
	defer w.leave(marker)

	arrayValues := make(map[int64]Value)
	fields := make([]Field, 0)
	key := luavm.Nil
	for {
		next, raw, err := table.Next(key)
		if err != nil {
			return Value{}, ErrInvalidValue
		}
		if next.IsNil() {
			break
		}
		item, err := w.fromLua(raw, depth+1)
		if err != nil {
			return Value{}, err
		}
		switch {
		case next.IsInt():
			if len(fields) != 0 || next.AsInt() < 1 {
				return Value{}, ErrInvalidValue
			}
			arrayValues[next.AsInt()] = item
		case next.IsString():
			if len(arrayValues) != 0 {
				return Value{}, ErrInvalidValue
			}
			fields = append(fields, Field{Key: next.AsString(), Value: item})
		default:
			return Value{}, ErrInvalidValue
		}
		key = next
	}

	if len(arrayValues) != 0 {
		items := make([]Value, len(arrayValues))
		for i := int64(1); i <= int64(len(arrayValues)); i++ {
			item, ok := arrayValues[i]
			if !ok {
				return Value{}, ErrInvalidValue
			}
			items[i-1] = item
		}
		return Value{Type: TypeArray, Array: items}, nil
	}
	return Normalize(Value{Type: TypeTable, Table: fields})
}

// ToLua reconstructs a fresh Lua value with no shared table identity.
func ToLua(runtime *luavm.VM, input Value) (luavm.Value, error) {
	if runtime == nil {
		return luavm.Nil, ErrInvalidValue
	}
	value, err := Normalize(input)
	if err != nil {
		return luavm.Nil, err
	}
	return toLua(value)
}

func toLua(value Value) (luavm.Value, error) {
	switch value.Type {
	case TypeNil:
		return luavm.Nil, nil
	case TypeBool:
		return luavm.NewBool(value.Boolean), nil
	case TypeInt:
		return luavm.NewInt(value.Integer), nil
	case TypeFloat:
		return luavm.NewFloat(value.Float), nil
	case TypeString:
		return luavm.NewString(value.String), nil
	case TypeArray:
		table := luavm.NewTableWithSize(len(value.Array), 0)
		for i := range value.Array {
			item, err := toLua(value.Array[i])
			if err != nil {
				return luavm.Nil, err
			}
			if err := table.Set(luavm.NewInt(int64(i+1)), item); err != nil {
				return luavm.Nil, err
			}
		}
		return luavm.NewTable(table), nil
	case TypeTable:
		table := luavm.NewTableWithSize(0, len(value.Table))
		for _, field := range value.Table {
			item, err := toLua(field.Value)
			if err != nil {
				return luavm.Nil, err
			}
			if err := table.Set(luavm.NewString(field.Key), item); err != nil {
				return luavm.Nil, err
			}
		}
		return luavm.NewTable(table), nil
	default:
		return luavm.Nil, ErrInvalidValue
	}
}
