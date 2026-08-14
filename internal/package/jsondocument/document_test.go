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
		`"\"\\"`:                `"\"\\"`,
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

func TestCanonicalNumberExponentBoundaries(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		input string
		want  string
	}{
		{input: `1e127`, want: "1" + strings.Repeat("0", 127)},
		{input: `-1e126`, want: "-1" + strings.Repeat("0", 126)},
		{input: `1e-126`, want: "0." + strings.Repeat("0", 125) + "1"},
		{input: `0e308`, want: "0"},
		{input: `1000000000000000000000000000000000000000000000000000000000000000e-63`, want: "1"},
	} {
		value, err := jsondocument.Parse([]byte(test.input))
		if err != nil {
			t.Fatalf("Parse(%s): %v", test.input, err)
		}
		if got := string(value.Canonical()); got != test.want {
			t.Fatalf("Canonical(%s) length=%d value=%q, want length=%d value=%q", test.input, len(got), got, len(test.want), test.want)
		}
		canonical := value.Canonical()
		if _, err := jsondocument.Parse(canonical); err != nil {
			t.Fatalf("canonical re-import %s: %v", test.input, err)
		}
	}
	for _, input := range []string{`1e128`, `-1e127`, `1e-127`, `1e308`, `1e-308`} {
		if _, err := jsondocument.Parse([]byte(input)); err == nil {
			t.Fatalf("non-roundtrippable canonical number %s accepted", input)
		}
	}
}

func TestCanonicalLimitedBoundaryAndExponentAmplification(t *testing.T) {
	value, err := jsondocument.Parse([]byte(`{"a":1}`))
	if err != nil {
		t.Fatal(err)
	}
	if got, err := value.CanonicalLimited(len(`{"a":1}`)); err != nil || string(got) != `{"a":1}` {
		t.Fatalf("exact boundary = %q, %v", got, err)
	}
	if _, err := value.CanonicalLimited(len(`{"a":1}`) - 1); err == nil {
		t.Fatal("over-limit canonical output accepted")
	}

	expanded, err := jsondocument.Parse([]byte("[" + strings.Repeat("1e127,", 9999) + "1e127]"))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := expanded.CanonicalLimited(1024); err == nil {
		t.Fatal("short exponent amplification passed bounded output")
	}
	allocations := testing.AllocsPerRun(5, func() {
		if _, err := expanded.CanonicalLimited(1024); err == nil {
			panic("expected bounded-output failure")
		}
	})
	if allocations > 100 {
		t.Fatalf("bounded exponent emission allocations = %.0f", allocations)
	}
}

func TestCanonicalLimitedLargeStringDoesNotAllocatePerRune(t *testing.T) {
	input := `"` + strings.Repeat("x", 1<<20) + `"`
	value, err := jsondocument.Parse([]byte(input))
	if err != nil {
		t.Fatal(err)
	}
	allocations := testing.AllocsPerRun(3, func() {
		result, err := value.CanonicalLimited(len(input))
		if err != nil || len(result) != len(input) {
			panic("large string canonicalization failed")
		}
	})
	if allocations > 100 {
		t.Fatalf("large string canonical allocations = %.0f", allocations)
	}
	if _, err := value.CanonicalLimited(8); err == nil {
		t.Fatal("large string unexpectedly fit tiny output limit")
	}
}

func TestParseExponentLengthCheckDoesNotExpandEveryNumber(t *testing.T) {
	plain := []byte("[" + strings.Repeat("12345,", 9999) + "12345]")
	exponents := []byte("[" + strings.Repeat("1e127,", 9999) + "1e127]")
	plainAllocations := testing.AllocsPerRun(3, func() {
		if _, err := jsondocument.Parse(plain); err != nil {
			panic(err)
		}
	})
	exponentAllocations := testing.AllocsPerRun(3, func() {
		if _, err := jsondocument.Parse(exponents); err != nil {
			panic(err)
		}
	})
	if exponentAllocations > plainAllocations+100 {
		t.Fatalf("exponent parse allocations %.0f, plain %.0f", exponentAllocations, plainAllocations)
	}
}

func TestComplexityDoesNotCopyLargeContainers(t *testing.T) {
	input := []byte("[" + strings.Repeat("null,", 99999) + "null]")
	value, err := jsondocument.Parse(input)
	if err != nil {
		t.Fatal(err)
	}
	metrics := value.Complexity()
	if metrics.Nodes != 100001 || metrics.ObjectMembers != 0 {
		t.Fatalf("metrics = %#v", metrics)
	}
	allocations := testing.AllocsPerRun(10, func() {
		if value.Complexity().Nodes != 100001 {
			panic("unexpected complexity")
		}
	})
	if allocations != 0 {
		t.Fatalf("complexity allocations = %.0f", allocations)
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
