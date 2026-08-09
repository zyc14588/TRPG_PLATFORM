// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest

import (
	"fmt"
	"strings"
	"testing"
	"unicode"
)

func TestValidPackageRelativePathSegmentGrammar(t *testing.T) {
	tests := []struct {
		name  string
		value string
		want  bool
	}{
		{name: "root Lua source", value: "main.lua", want: true},
		{name: "nested Lua source", value: "lua/main.lua", want: true},
		{name: "deeply nested Lua source", value: "lua/sub/main.lua", want: true},
		{name: "ordinary repeated dots segment", value: ".../x.lua", want: true},
		{name: "ordinary embedded dots segment", value: "foo..bar/x.lua", want: true},

		{name: "upper-case Windows drive", value: "C:/escape.lua"},
		{name: "Windows drive with backslashes", value: `C:\escape.lua`},
		{name: "Windows drive-relative", value: "C:relative.lua"},
		{name: "embedded Windows drive", value: "a/C:/escape.lua"},
		{name: "embedded drive-relative", value: "a/z:relative.lua"},
		{name: "colon before separator", value: "foo:/bar.lua"},

		{name: "colon inside root segment", value: "x:y.lua"},
		{name: "colon inside nested segment", value: "a/x:y.lua"},
		{name: "colon inside middle segment", value: "a/b:c/d.lua"},
		{name: "colon-only first segment", value: ":/foo.lua"},
		{name: "colon-only middle segment", value: "a/:/b.lua"},
		{name: "trailing colon middle segment", value: "a/b:/c.lua"},
		{name: "multi-colon middle segment", value: "a/::/b.lua"},

		{name: "slash UNC", value: "//server/share/a.lua"},
		{name: "backslash UNC", value: `\\server\share\a.lua`},
		{name: "Windows device namespace", value: `\\?\C:\a.lua`},
		{name: "Windows local device namespace", value: `\\.\C:\a.lua`},

		{name: "absolute", value: "/main.lua"},
		{name: "parent traversal", value: "../main.lua"},
		{name: "nested parent traversal", value: "a/../../main.lua"},
		{name: "cleaning traversal", value: "lua/../main.lua"},
		{name: "leading dot segment", value: "./main.lua"},
		{name: "nested dot segment", value: "lua/./main.lua"},
		{name: "empty segment", value: "lua//main.lua"},

		{name: "NUL", value: "lua/\x00.lua"},
		{name: "C0 SOH", value: "lua/\x01.lua"},
		{name: "C0 unit separator", value: "lua/\x1f.lua"},
		{name: "DEL", value: "lua/\x7f.lua"},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			if got := validPackageRelativePath(test.value); got != test.want {
				t.Fatalf("validPackageRelativePath(%q) = %t, want %t", test.value, got, test.want)
			}
		})
	}
}

func TestValidPackageRelativePathChecksEverySegment(t *testing.T) {
	illegalSegments := []string{"C:", "z:relative", "foo:", "x:y", ":", "::"}
	positions := []struct {
		name  string
		place func(string) string
	}{
		{name: "first", place: func(segment string) string { return segment + "/x.lua" }},
		{name: "middle", place: func(segment string) string { return "a/" + segment + "/x.lua" }},
		{name: "last", place: func(segment string) string { return "a/b/" + segment }},
	}
	checked := 0
	for _, position := range positions {
		position := position
		for index, illegalSegment := range illegalSegments {
			illegalSegment := illegalSegment
			t.Run(fmt.Sprintf("%s/%d", position.name, index), func(t *testing.T) {
				candidate := position.place(illegalSegment)
				if validPackageRelativePath(candidate) {
					t.Fatalf("illegal segment %q at %s position accepted in %q", illegalSegment, position.name, candidate)
				}
			})
			checked++
		}
	}
	t.Logf("checked %d illegal segment-position combinations", checked)
}

func TestValidPackageRelativePathBoundedProperties(t *testing.T) {
	contexts := [][]string{
		{"pkg", "lua", "main.lua"},
		{"...", "module", "entry.lua"},
		{"foo..bar", "nested", "x.lua"},
		{"包", "脚本", "入口.lua"},
	}
	illegalSegments := []string{"C:", "z:relative", "foo:", "x:y", ":", "::", ".", "..", "", `bad\segment`}
	candidates := []string{
		"main.lua",
		"lua/main.lua",
		".../x.lua",
		"foo..bar/x.lua",
		"包/脚本/入口.lua",
		"/root.lua",
		"//server/share/a.lua",
		`\\?\C:\a.lua`,
		"a/\x00/b.lua",
		"a/\x1f/b.lua",
		"a/\x7f/b.lua",
	}

	generated := 0
	for _, context := range contexts {
		for position := range context {
			for _, illegalSegment := range illegalSegments {
				segments := append([]string(nil), context...)
				segments[position] = illegalSegment
				candidate := strings.Join(segments, "/")
				candidates = append(candidates, candidate)
				generated++
				if validPackageRelativePath(candidate) {
					t.Fatalf("generated path with illegal segment %q at position %d accepted: %q", illegalSegment, position, candidate)
				}
			}
		}
	}

	for _, candidate := range candidates {
		if validPackageRelativePath(candidate) {
			assertAcceptedPackagePathInvariants(t, candidate)
		}
	}
	t.Logf("checked %d deterministic illegal insertions across four three-segment contexts", generated)
}

func assertAcceptedPackagePathInvariants(t *testing.T, value string) {
	t.Helper()
	if value == "" || strings.HasPrefix(value, "/") || strings.Contains(value, `\`) {
		t.Fatalf("accepted path has root, empty, or device interpretation: %q", value)
	}
	segments := strings.Split(value, "/")
	for _, segment := range segments {
		if segment == "" || segment == "." || segment == ".." {
			t.Fatalf("accepted path has an empty, dot, or escaping segment: %q", value)
		}
		if strings.ContainsAny(segment, `:/\`) {
			t.Fatalf("accepted path segment has a separator or volume/device interpretation: %q", value)
		}
		for _, character := range segment {
			if unicode.IsControl(character) {
				t.Fatalf("accepted path contains control character %U: %q", character, value)
			}
		}
	}
}
