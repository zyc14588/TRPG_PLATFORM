// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package jsondocument implements the bounded JSON value contract shared by
// package extensions, the Host preservation model, and Creator Studio.
package jsondocument

import (
	"bytes"
	"fmt"
	"encoding/json"
	"sort"
	"strconv"
	"strings"
	"unicode/utf16"
	"unicode/utf8"
)

const (
	MaxDepth       = 64
	MaxNumberBytes = 128
	MaxExponent    = 308
)

type Kind uint8

const (
	Null Kind = iota
	Boolean
	Number
	String
	Array
	Object
)

type Member struct {
	Name  string
	Value Value
}

// Value is an immutable-by-API JSON tree. Number preserves its exact token.
type Value struct {
	kind    Kind
	boolean bool
	text    string
	array   []Value
	object  []Member
}

func (value Value) Kind() Kind                  { return value.kind }
func (value Value) Bool() (bool, bool)          { return value.boolean, value.kind == Boolean }
func (value Value) Text() (string, bool)        { return value.text, value.kind == String }
func (value Value) NumberToken() (string, bool) { return value.text, value.kind == Number }

func (value Value) Elements() []Value {
	return append([]Value(nil), value.array...)
}

func (value Value) Members() []Member {
	result := make([]Member, len(value.object))
	copy(result, value.object)
	return result
}

func (value Value) Lookup(name string) (Value, bool) {
	for _, member := range value.object {
		if member.Name == name {
			return member.Value, true
		}
	}
	return Value{}, false
}

// Interface returns the representation expected by the pinned schema
// validator while retaining exact decimal tokens as json.Number.
func (value Value) Interface() any {
	switch value.kind {
	case Null: return nil
	case Boolean: return value.boolean
	case Number: return json.Number(value.text)
	case String: return value.text
	case Array:
		result := make([]any, len(value.array)); for i := range value.array { result[i] = value.array[i].Interface() }; return result
	case Object:
		result := make(map[string]any, len(value.object)); for _, member := range value.object { result[member.Name] = member.Value.Interface() }; return result
	default: return nil
	}
}

// Parse rejects duplicate keys, invalid Unicode, excess nesting, lossy number
// forms, and trailing values before returning a canonicalizable tree.
func Parse(data []byte) (Value, error) {
	if len(data) == 0 {
		return Value{}, fmt.Errorf("JSON document is empty")
	}
	if bytes.HasPrefix(data, []byte{0xef, 0xbb, 0xbf}) {
		return Value{}, fmt.Errorf("JSON document must not contain a BOM")
	}
	if !utf8.Valid(data) {
		return Value{}, fmt.Errorf("JSON document is not valid UTF-8")
	}
	parser := parser{data: data}
	value, err := parser.value(0)
	if err != nil {
		return Value{}, err
	}
	parser.space()
	if parser.position != len(data) {
		return Value{}, parser.fail("trailing JSON value or token")
	}
	return value, nil
}

// Canonical emits stable JSON: sorted object names, minimal strings and
// decimal numbers, no whitespace, and unchanged array order.
func (value Value) Canonical() []byte {
	var output bytes.Buffer
	value.appendCanonical(&output)
	return output.Bytes()
}

func (value Value) appendCanonical(output *bytes.Buffer) {
	switch value.kind {
	case Null:
		output.WriteString("null")
	case Boolean:
		output.WriteString(strconv.FormatBool(value.boolean))
	case Number:
		output.WriteString(canonicalNumber(value.text))
	case String:
		appendString(output, value.text)
	case Array:
		output.WriteByte('[')
		for index, child := range value.array {
			if index > 0 {
				output.WriteByte(',')
			}
			child.appendCanonical(output)
		}
		output.WriteByte(']')
	case Object:
		members := value.Members()
		slicesSortMembers(members)
		output.WriteByte('{')
		for index, member := range members {
			if index > 0 {
				output.WriteByte(',')
			}
			appendString(output, member.Name)
			output.WriteByte(':')
			member.Value.appendCanonical(output)
		}
		output.WriteByte('}')
	}
}

func slicesSortMembers(values []Member) {
	sort.Slice(values, func(i, j int) bool { return values[i].Name < values[j].Name })
}

func appendString(output *bytes.Buffer, value string) {
	output.WriteByte('"')
	for _, character := range value {
		switch character {
		case '"', '\\':
			output.WriteByte('\\')
			output.WriteRune(character)
		case '\b':
			output.WriteString(`\b`)
		case '\f':
			output.WriteString(`\f`)
		case '\n':
			output.WriteString(`\n`)
		case '\r':
			output.WriteString(`\r`)
		case '\t':
			output.WriteString(`\t`)
		default:
			if character < 0x20 {
				fmt.Fprintf(output, `\u%04x`, character)
			} else {
				output.WriteRune(character)
			}
		}
	}
	output.WriteByte('"')
}

type parser struct {
	data     []byte
	position int
}

func (parser *parser) value(depth int) (Value, error) {
	parser.space()
	if parser.position >= len(parser.data) {
		return Value{}, parser.fail("expected JSON value")
	}
	switch parser.data[parser.position] {
	case 'n':
		if !parser.literal("null") {
			return Value{}, parser.fail("invalid null literal")
		}
		return Value{kind: Null}, nil
	case 't':
		if !parser.literal("true") {
			return Value{}, parser.fail("invalid true literal")
		}
		return Value{kind: Boolean, boolean: true}, nil
	case 'f':
		if !parser.literal("false") {
			return Value{}, parser.fail("invalid false literal")
		}
		return Value{kind: Boolean}, nil
	case '"':
		text, err := parser.string()
		return Value{kind: String, text: text}, err
	case '[':
		return parser.arrayValue(depth)
	case '{':
		return parser.objectValue(depth)
	default:
		if parser.data[parser.position] == '-' || isDigit(parser.data[parser.position]) {
			token, err := parser.number()
			return Value{kind: Number, text: token}, err
		}
		return Value{}, parser.fail("unexpected JSON token")
	}
}

func (parser *parser) arrayValue(depth int) (Value, error) {
	if depth >= MaxDepth {
		return Value{}, parser.fail("JSON nesting exceeds 64")
	}
	parser.position++
	parser.space()
	values := make([]Value, 0)
	if parser.consume(']') {
		return Value{kind: Array, array: values}, nil
	}
	for {
		child, err := parser.value(depth + 1)
		if err != nil {
			return Value{}, err
		}
		values = append(values, child)
		parser.space()
		if parser.consume(']') {
			break
		}
		if !parser.consume(',') {
			return Value{}, parser.fail("array requires comma or closing bracket")
		}
	}
	return Value{kind: Array, array: values}, nil
}

func (parser *parser) objectValue(depth int) (Value, error) {
	if depth >= MaxDepth {
		return Value{}, parser.fail("JSON nesting exceeds 64")
	}
	parser.position++
	parser.space()
	members := make([]Member, 0)
	seen := make(map[string]struct{})
	if parser.consume('}') {
		return Value{kind: Object, object: members}, nil
	}
	for {
		parser.space()
		if parser.position >= len(parser.data) || parser.data[parser.position] != '"' {
			return Value{}, parser.fail("object member name must be a string")
		}
		name, err := parser.string()
		if err != nil {
			return Value{}, err
		}
		if _, exists := seen[name]; exists {
			return Value{}, parser.fail("duplicate object member " + strconv.Quote(name))
		}
		seen[name] = struct{}{}
		parser.space()
		if !parser.consume(':') {
			return Value{}, parser.fail("object member requires colon")
		}
		child, err := parser.value(depth + 1)
		if err != nil {
			return Value{}, err
		}
		members = append(members, Member{Name: name, Value: child})
		parser.space()
		if parser.consume('}') {
			break
		}
		if !parser.consume(',') {
			return Value{}, parser.fail("object requires comma or closing brace")
		}
	}
	return Value{kind: Object, object: members}, nil
}

func (parser *parser) string() (string, error) {
	parser.position++
	var output strings.Builder
	for parser.position < len(parser.data) {
		character := parser.data[parser.position]
		if character == '"' {
			parser.position++
			return output.String(), nil
		}
		if character < 0x20 {
			return "", parser.fail("unescaped control character in string")
		}
		if character != '\\' {
			runeValue, width := utf8.DecodeRune(parser.data[parser.position:])
			output.WriteRune(runeValue)
			parser.position += width
			continue
		}
		parser.position++
		if parser.position >= len(parser.data) {
			return "", parser.fail("unfinished string escape")
		}
		escape := parser.data[parser.position]
		parser.position++
		switch escape {
		case '"', '\\', '/':
			output.WriteByte(escape)
		case 'b':
			output.WriteByte('\b')
		case 'f':
			output.WriteByte('\f')
		case 'n':
			output.WriteByte('\n')
		case 'r':
			output.WriteByte('\r')
		case 't':
			output.WriteByte('\t')
		case 'u':
			first, err := parser.hexRune()
			if err != nil {
				return "", err
			}
			if 0xd800 <= first && first <= 0xdbff {
				if parser.position+2 > len(parser.data) || parser.data[parser.position] != '\\' || parser.data[parser.position+1] != 'u' {
					return "", parser.fail("high surrogate requires a low surrogate")
				}
				parser.position += 2
				second, err := parser.hexRune()
				if err != nil {
					return "", err
				}
				if second < 0xdc00 || second > 0xdfff {
					return "", parser.fail("invalid low surrogate")
				}
				output.WriteRune(utf16.DecodeRune(first, second))
			} else if 0xdc00 <= first && first <= 0xdfff {
				return "", parser.fail("unpaired low surrogate")
			} else {
				output.WriteRune(first)
			}
		default:
			return "", parser.fail("invalid string escape")
		}
	}
	return "", parser.fail("unterminated string")
}

func (parser *parser) hexRune() (rune, error) {
	if parser.position+4 > len(parser.data) {
		return 0, parser.fail("short Unicode escape")
	}
	var value rune
	for _, character := range parser.data[parser.position : parser.position+4] {
		value <<= 4
		switch {
		case '0' <= character && character <= '9':
			value += rune(character - '0')
		case 'a' <= character && character <= 'f':
			value += rune(character-'a') + 10
		case 'A' <= character && character <= 'F':
			value += rune(character-'A') + 10
		default:
			return 0, parser.fail("invalid Unicode escape")
		}
	}
	parser.position += 4
	return value, nil
}

func (parser *parser) number() (string, error) {
	start := parser.position
	parser.consume('-')
	if parser.position-start > MaxNumberBytes {
		return "", parser.fail("number token exceeds 128 bytes")
	}
	if parser.position >= len(parser.data) {
		return "", parser.fail("incomplete number")
	}
	if parser.consume('0') {
		if parser.position < len(parser.data) && isDigit(parser.data[parser.position]) {
			return "", parser.fail("number has a leading zero")
		}
	} else {
		if parser.position >= len(parser.data) || parser.data[parser.position] < '1' || parser.data[parser.position] > '9' {
			return "", parser.fail("invalid integer")
		}
		for parser.position < len(parser.data) && isDigit(parser.data[parser.position]) {
			parser.position++
			if parser.position-start > MaxNumberBytes {
				return "", parser.fail("number token exceeds 128 bytes")
			}
		}
	}
	if parser.consume('.') {
		if parser.position-start > MaxNumberBytes {
			return "", parser.fail("number token exceeds 128 bytes")
		}
		fraction := parser.position
		for parser.position < len(parser.data) && isDigit(parser.data[parser.position]) {
			parser.position++
			if parser.position-start > MaxNumberBytes {
				return "", parser.fail("number token exceeds 128 bytes")
			}
		}
		if parser.position == fraction {
			return "", parser.fail("fraction requires digits")
		}
	}
	if parser.position < len(parser.data) && (parser.data[parser.position] == 'e' || parser.data[parser.position] == 'E') {
		parser.position++
		if parser.position-start > MaxNumberBytes {
			return "", parser.fail("number token exceeds 128 bytes")
		}
		sign := 1
		if parser.consume('+') { /* positive */
		} else if parser.consume('-') {
			sign = -1
		}
		if parser.position-start > MaxNumberBytes {
			return "", parser.fail("number token exceeds 128 bytes")
		}
		exponentStart := parser.position
		exponent := 0
		significantDigits := 0
		for parser.position < len(parser.data) && isDigit(parser.data[parser.position]) {
			digit := int(parser.data[parser.position] - '0')
			parser.position++
			if parser.position-start > MaxNumberBytes {
				return "", parser.fail("number token exceeds 128 bytes")
			}
			if significantDigits > 0 || digit != 0 {
				significantDigits++
				if significantDigits > 3 {
					return "", parser.fail("number exponent exceeds absolute 308")
				}
			}
			exponent = exponent*10 + digit
			if exponent > MaxExponent {
				return "", parser.fail("number exponent exceeds absolute 308")
			}
		}
		if parser.position == exponentStart {
			return "", parser.fail("exponent requires digits")
		}
		if exponent*sign > MaxExponent || exponent*sign < -MaxExponent {
			return "", parser.fail("number exponent exceeds absolute 308")
		}
	}
	if parser.position-start > MaxNumberBytes {
		return "", parser.fail("number token exceeds 128 bytes")
	}
	return string(parser.data[start:parser.position]), nil
}

func canonicalNumber(token string) string {
	negative := strings.HasPrefix(token, "-")
	if negative {
		token = token[1:]
	}
	exponent := 0
	if index := strings.IndexAny(token, "eE"); index >= 0 {
		exponent, _ = strconv.Atoi(token[index+1:])
		token = token[:index]
	}
	integer, fraction := token, ""
	if index := strings.IndexByte(token, '.'); index >= 0 {
		integer, fraction = token[:index], token[index+1:]
	}
	digits := integer + fraction
	decimal := len(integer) + exponent
	leading := 0
	for leading < len(digits) && digits[leading] == '0' {
		leading++
	}
	if leading == len(digits) {
		return "0"
	}
	digits = digits[leading:]
	decimal -= leading
	for len(digits) > 1 && digits[len(digits)-1] == '0' {
		digits = digits[:len(digits)-1]
	}
	var result string
	switch {
	case decimal <= 0:
		result = "0." + strings.Repeat("0", -decimal) + digits
	case decimal >= len(digits):
		result = digits + strings.Repeat("0", decimal-len(digits))
	default:
		result = digits[:decimal] + "." + digits[decimal:]
	}
	if negative {
		return "-" + result
	}
	return result
}

func (parser *parser) literal(value string) bool {
	if !bytes.HasPrefix(parser.data[parser.position:], []byte(value)) {
		return false
	}
	parser.position += len(value)
	return true
}

func (parser *parser) consume(character byte) bool {
	if parser.position < len(parser.data) && parser.data[parser.position] == character {
		parser.position++
		return true
	}
	return false
}

func (parser *parser) space() {
	for parser.position < len(parser.data) {
		switch parser.data[parser.position] {
		case ' ', '\t', '\r', '\n':
			parser.position++
		default:
			return
		}
	}
}

func (parser *parser) fail(message string) error {
	return fmt.Errorf("%s at byte %d", message, parser.position)
}

func isDigit(value byte) bool { return '0' <= value && value <= '9' }
