// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package jsondocument_test

import (
	"fmt"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/jsondocument"
)

func TestCanonical(t *testing.T) {
	t.Parallel()
	input := []byte(` {"z":1.2300e+2,"a":"\ud83d\ude00","items":[-0,1e-2]} `)
	value, err := jsondocument.Parse(input)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := string(value.Canonical()), `{"a":"😀","items":[0,0.01],"z":123}`; got != want {
		t.Fatalf("canonical = %s, want %s", got, want)
	}
}

func TestCanonicalGoldenVectors(t *testing.T) {
	t.Parallel()
	for input, want := range map[string]string{
		`-0`: `0`, `0.000`: `0`, `1e3`: `1000`, `1000e-3`: `1`,
		`1.2300`: `1.23`, `1e-3`: `0.001`, `-12.50e+2`: `-1250`,
		`"\u0061\/b\b\f\n\r\t"`: `"a/b\b\f\n\r\t"`,
	} {
		value, err := jsondocument.Parse([]byte(input))
		if err != nil {
			t.Fatalf("Parse(%s): %v", input, err)
		}
		if got := string(value.Canonical()); got != want {
			t.Errorf("Canonical(%s) = %s, want %s", input, got, want)
		}
	}
}

func TestRejectsUnsafeJSON(t *testing.T) {
	t.Parallel()
	tooDeep := strings.Repeat("[", jsondocument.MaxDepth+1) + "0" + strings.Repeat("]", jsondocument.MaxDepth+1)
	for _, input := range [][]byte{
		[]byte(`{"a":1,"\u0061":2}`),
		[]byte(`"\ud800"`),
		[]byte(`01`),
		[]byte(`1e309`),
		[]byte(tooDeep),
		{0xff},
		{0xef, 0xbb, 0xbf, '0'},
	} {
		if _, err := jsondocument.Parse(input); err == nil {
			t.Errorf("Parse(%q) unexpectedly succeeded", input)
		}
	}
}

func TestDepthBoundary(t *testing.T) {
	t.Parallel()
	accepted := strings.Repeat("[", jsondocument.MaxDepth) + "0" + strings.Repeat("]", jsondocument.MaxDepth)
	if _, err := jsondocument.Parse([]byte(accepted)); err != nil {
		t.Fatalf("64 containers: %v", err)
	}
	rejected := strings.Repeat("[", jsondocument.MaxDepth+1) + "0" + strings.Repeat("]", jsondocument.MaxDepth+1)
	if _, err := jsondocument.Parse([]byte(rejected)); err == nil {
		t.Fatal("65 containers accepted")
	}
}

func TestLargeFlatObjectCanonicalizes(t *testing.T) {
	t.Parallel()
	var input strings.Builder
	input.WriteByte('{')
	for index := 9999; index >= 0; index-- {
		if index != 9999 {
			input.WriteByte(',')
		}
		fmt.Fprintf(&input, `"key%05d":%d`, index, index)
	}
	input.WriteByte('}')
	value, err := jsondocument.Parse([]byte(input.String()))
	if err != nil {
		t.Fatal(err)
	}
	canonical := string(value.Canonical())
	if !strings.HasPrefix(canonical, `{"key00000":0,"key00001":1`) || !strings.HasSuffix(canonical, `"key09999":9999}`) {
		t.Fatal("large object was not sorted canonically")
	}
}

func FuzzParse(f *testing.F) {
	f.Add([]byte(`{"third.party.probe":true}`))
	f.Fuzz(func(t *testing.T, data []byte) {
		value, err := jsondocument.Parse(data)
		if err != nil {
			return
		}
		canonical := value.Canonical()
		second, err := jsondocument.Parse(canonical)
		if err != nil {
			t.Fatalf("canonical parse: %v", err)
		}
		if string(second.Canonical()) != string(canonical) {
			t.Fatal("canonical form is not idempotent")
		}
	})
}
