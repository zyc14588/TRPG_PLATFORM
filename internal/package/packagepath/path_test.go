// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package packagepath_test

import (
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/packagepath"
)

func TestValidate(t *testing.T) {
	t.Parallel()
	for _, value := range []string{"package.toml", "extensions/third.party.probe/payload.json", "内容/item.json"} {
		if err := packagepath.Validate(value); err != nil {
			t.Fatalf("Validate(%q): %v", value, err)
		}
	}
	for _, value := range []string{"", "/etc/passwd", "../escape", "a/../b", `a\b`, "C:/x", "a//b", "a/./b", " e", "e ", "e.", "CON", "nul.json", "COM1.txt", "LPT9", "e\x00x", "e\u0301.json", strings.Repeat("a", packagepath.MaxBytes+1)} {
		if err := packagepath.Validate(value); err == nil {
			t.Errorf("Validate(%q) unexpectedly succeeded", value)
		}
	}
}

func TestSetRejectsPortableCollisions(t *testing.T) {
	t.Parallel()
	var set packagepath.Set
	if err := set.Add("extensions/third.party.probe/A.json"); err != nil {
		t.Fatal(err)
	}
	if err := set.Add("extensions/third.party.probe/a.json"); err == nil {
		t.Fatal("case-only collision accepted")
	}
	var unicodeSet packagepath.Set
	if err := unicodeSet.Add("extensions/third.party.probe/ß.json"); err != nil {
		t.Fatal(err)
	}
	if err := unicodeSet.Add("extensions/third.party.probe/ss.json"); err == nil {
		t.Fatal("Unicode fold collision accepted")
	}
}
