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
	for _, value := range []string{
		"", "/etc/passwd", "../escape", "a/../b", `a\b`, "C:/x", "a//b", "a/./b",
		" e", "e ", "e.", "CON", "nul.json", "COM1.txt", "COM1 .txt", "LPT9", "COM\u00b9.log", "lpt\u00b3",
		"CONIN$", "conout$.txt", "a<b", "a>b", `a"b`, "a|b", "a?b", "a*b",
		"e\x00x", "e\u0301.json", strings.Repeat("a", packagepath.MaxBytes+1),
	} {
		if err := packagepath.Validate(value); err == nil {
			t.Errorf("Validate(%q) unexpectedly succeeded", value)
		}
	}
}

func TestTreeSetRejectsFileTreeConflicts(t *testing.T) {
	t.Parallel()
	var ancestor packagepath.TreeSet
	if err := ancestor.Add("Data/File.json"); err != nil {
		t.Fatal(err)
	}
	if err := ancestor.Add("data/file.json/child"); err == nil {
		t.Fatal("regular-file ancestor accepted")
	}

	var descendant packagepath.TreeSet
	if err := descendant.Add("data/file.json/child"); err != nil {
		t.Fatal(err)
	}
	if err := descendant.Add("DATA/FILE.JSON"); err == nil {
		t.Fatal("regular-file descendant conflict accepted")
	}
}

func TestTreeSetRejectsAliasedDirectoryComponents(t *testing.T) {
	t.Parallel()
	var ascii packagepath.TreeSet
	if err := ascii.AddFile("A/x.json"); err != nil {
		t.Fatal(err)
	}
	if err := ascii.AddFile("a/y.json"); err == nil {
		t.Fatal("case-aliased parent directory accepted")
	}

	var unicode packagepath.TreeSet
	if err := unicode.AddDirectory("Straße"); err != nil {
		t.Fatal(err)
	}
	if err := unicode.AddFile("STRASSE/y.json"); err == nil {
		t.Fatal("Unicode-folded parent directory alias accepted")
	}
}

func TestTreeSetDirectorySemantics(t *testing.T) {
	t.Parallel()
	var tree packagepath.TreeSet
	if err := tree.AddDirectory("data"); err != nil {
		t.Fatal(err)
	}
	if err := tree.AddDirectory("data"); err != nil {
		t.Fatalf("repeated directory: %v", err)
	}
	if err := tree.AddFile("data/item.json"); err != nil {
		t.Fatal(err)
	}
	if err := tree.AddFile("data"); err == nil {
		t.Fatal("directory replaced by regular file")
	}
	if err := tree.AddDirectory("data/item.json"); err == nil {
		t.Fatal("regular file replaced by directory")
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
	var renormalized packagepath.Set
	if err := renormalized.Add("extensions/third.party.probe/İ.json"); err != nil {
		t.Fatal(err)
	}
	if err := renormalized.Add("extensions/third.party.probe/i\u0307.json"); err == nil {
		t.Fatal("post-fold normalization collision accepted")
	}
}
