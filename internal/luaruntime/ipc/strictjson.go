// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"fmt"
	"unicode/utf16"
	"unicode/utf8"
)

const maxStrictJSONDepth = 512

type strictJSONScanner struct {
	data  []byte
	index int
}

func validateStrictJSON(data []byte) error {
	if len(data) == 0 || !utf8.Valid(data) {
		return ErrInvalidMessage
	}
	scanner := strictJSONScanner{data: data}
	if err := scanner.value(0); err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidMessage, err)
	}
	scanner.space()
	if scanner.index != len(scanner.data) {
		return fmt.Errorf("%w: trailing JSON", ErrInvalidMessage)
	}
	return nil
}

func (s *strictJSONScanner) value(depth int) error {
	if depth > maxStrictJSONDepth {
		return fmt.Errorf("JSON nesting exceeds %d", maxStrictJSONDepth)
	}
	s.space()
	if s.index >= len(s.data) {
		return fmt.Errorf("unexpected end of JSON")
	}
	switch s.data[s.index] {
	case '{':
		return s.object(depth + 1)
	case '[':
		return s.array(depth + 1)
	case '"':
		_, err := s.string()
		return err
	case 't':
		return s.literal("true")
	case 'f':
		return s.literal("false")
	case 'n':
		return s.literal("null")
	default:
		return s.number()
	}
}

func (s *strictJSONScanner) object(depth int) error {
	s.index++
	s.space()
	if s.take('}') {
		return nil
	}
	keys := make(map[string]struct{})
	for {
		s.space()
		if s.index >= len(s.data) || s.data[s.index] != '"' {
			return fmt.Errorf("object key must be a string")
		}
		key, err := s.string()
		if err != nil {
			return err
		}
		if _, exists := keys[key]; exists {
			return fmt.Errorf("duplicate object key %q", key)
		}
		keys[key] = struct{}{}
		s.space()
		if !s.take(':') {
			return fmt.Errorf("missing colon after object key")
		}
		if err := s.value(depth); err != nil {
			return err
		}
		s.space()
		if s.take('}') {
			return nil
		}
		if !s.take(',') {
			return fmt.Errorf("missing comma between object members")
		}
	}
}

func (s *strictJSONScanner) array(depth int) error {
	s.index++
	s.space()
	if s.take(']') {
		return nil
	}
	for {
		if err := s.value(depth); err != nil {
			return err
		}
		s.space()
		if s.take(']') {
			return nil
		}
		if !s.take(',') {
			return fmt.Errorf("missing comma between array elements")
		}
	}
}

func (s *strictJSONScanner) string() (string, error) {
	if !s.take('"') {
		return "", fmt.Errorf("expected JSON string")
	}
	decoded := make([]byte, 0, 32)
	for s.index < len(s.data) {
		current := s.data[s.index]
		switch {
		case current == '"':
			s.index++
			return string(decoded), nil
		case current == '\\':
			s.index++
			if s.index >= len(s.data) {
				return "", fmt.Errorf("incomplete JSON escape")
			}
			escape := s.data[s.index]
			s.index++
			switch escape {
			case '"', '\\', '/':
				decoded = append(decoded, escape)
			case 'b':
				decoded = append(decoded, '\b')
			case 'f':
				decoded = append(decoded, '\f')
			case 'n':
				decoded = append(decoded, '\n')
			case 'r':
				decoded = append(decoded, '\r')
			case 't':
				decoded = append(decoded, '\t')
			case 'u':
				codeUnit, err := s.hexCodeUnit()
				if err != nil {
					return "", err
				}
				var scalar rune
				switch {
				case codeUnit >= 0xd800 && codeUnit <= 0xdbff:
					if s.index+2 > len(s.data) || s.data[s.index] != '\\' || s.data[s.index+1] != 'u' {
						return "", fmt.Errorf("lone high surrogate")
					}
					s.index += 2
					low, err := s.hexCodeUnit()
					if err != nil {
						return "", err
					}
					if low < 0xdc00 || low > 0xdfff {
						return "", fmt.Errorf("invalid surrogate pair")
					}
					scalar = utf16.DecodeRune(rune(codeUnit), rune(low))
				case codeUnit >= 0xdc00 && codeUnit <= 0xdfff:
					return "", fmt.Errorf("lone low surrogate")
				default:
					scalar = rune(codeUnit)
				}
				decoded = utf8.AppendRune(decoded, scalar)
			default:
				return "", fmt.Errorf("invalid JSON escape")
			}
		case current < 0x20:
			return "", fmt.Errorf("unescaped control character in JSON string")
		default:
			scalar, size := utf8.DecodeRune(s.data[s.index:])
			if scalar == utf8.RuneError && size == 1 {
				return "", fmt.Errorf("invalid UTF-8 in JSON string")
			}
			decoded = append(decoded, s.data[s.index:s.index+size]...)
			s.index += size
		}
	}
	return "", fmt.Errorf("unterminated JSON string")
}

func (s *strictJSONScanner) hexCodeUnit() (uint16, error) {
	if s.index+4 > len(s.data) {
		return 0, fmt.Errorf("incomplete Unicode escape")
	}
	var value uint16
	for range 4 {
		value <<= 4
		current := s.data[s.index]
		s.index++
		switch {
		case current >= '0' && current <= '9':
			value |= uint16(current - '0')
		case current >= 'a' && current <= 'f':
			value |= uint16(current-'a') + 10
		case current >= 'A' && current <= 'F':
			value |= uint16(current-'A') + 10
		default:
			return 0, fmt.Errorf("invalid Unicode escape")
		}
	}
	return value, nil
}

func (s *strictJSONScanner) number() error {
	start := s.index
	if s.take('-') && s.index >= len(s.data) {
		return fmt.Errorf("incomplete JSON number")
	}
	if s.take('0') {
		if s.index < len(s.data) && isDigit(s.data[s.index]) {
			return fmt.Errorf("leading zero in JSON number")
		}
	} else {
		if s.index >= len(s.data) || s.data[s.index] < '1' || s.data[s.index] > '9' {
			return fmt.Errorf("invalid JSON value")
		}
		for s.index < len(s.data) && isDigit(s.data[s.index]) {
			s.index++
		}
	}
	if s.take('.') {
		if s.index >= len(s.data) || !isDigit(s.data[s.index]) {
			return fmt.Errorf("invalid JSON fraction")
		}
		for s.index < len(s.data) && isDigit(s.data[s.index]) {
			s.index++
		}
	}
	if s.index < len(s.data) && (s.data[s.index] == 'e' || s.data[s.index] == 'E') {
		s.index++
		if s.index < len(s.data) && (s.data[s.index] == '+' || s.data[s.index] == '-') {
			s.index++
		}
		if s.index >= len(s.data) || !isDigit(s.data[s.index]) {
			return fmt.Errorf("invalid JSON exponent")
		}
		for s.index < len(s.data) && isDigit(s.data[s.index]) {
			s.index++
		}
	}
	if s.index == start {
		return fmt.Errorf("invalid JSON number")
	}
	return nil
}

func (s *strictJSONScanner) literal(value string) error {
	if s.index+len(value) > len(s.data) || string(s.data[s.index:s.index+len(value)]) != value {
		return fmt.Errorf("invalid JSON literal")
	}
	s.index += len(value)
	return nil
}

func (s *strictJSONScanner) space() {
	for s.index < len(s.data) {
		switch s.data[s.index] {
		case ' ', '\t', '\r', '\n':
			s.index++
		default:
			return
		}
	}
}

func (s *strictJSONScanner) take(expected byte) bool {
	if s.index < len(s.data) && s.data[s.index] == expected {
		s.index++
		return true
	}
	return false
}

func isDigit(value byte) bool {
	return value >= '0' && value <= '9'
}
