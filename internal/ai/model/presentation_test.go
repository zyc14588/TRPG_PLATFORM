// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model

import (
	"strings"
	"testing"
)

func TestPresentationTrustedLabelBounds(t *testing.T) {
	for _, v := range []string{"", strings.Repeat("界", 81), "label\nprivate", string([]byte{255}), "   "} {
		if presentationLabel(v) {
			t.Fatal("invalid annotation accepted")
		}
	}
	for _, v := range []string{"Verified model", strings.Repeat("界", 80), "selected"} {
		if !presentationLabel(v) {
			t.Fatal("bounded annotation rejected")
		}
	}
	if _, e := NewPresentationSource(nil, map[string]string{"w/fabricated": "Claimed ready"}); e == nil {
		t.Fatal("annotation manufactured a registry")
	}
}
