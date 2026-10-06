// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package store

import (
	"encoding/json"
	"fmt"
	"strings"
	"testing"
)

func TestWorkspaceAccessCannotBeClaimedOrChangedByCaller(t *testing.T) {
	credential := Credential("fixture-secret-credential-a")
	rows := []Membership{{Principal: "alice", Workspace: "a", Install: true, Read: true}}
	a, err := NewAccess(map[Credential][]Membership{credential: rows})
	if err != nil {
		t.Fatal(err)
	}
	rows[0].Workspace = "b"
	if _, err = a.Authorize(credential, "a", true); err != nil {
		t.Fatal(err)
	}
	for _, c := range []Credential{Credential("alice"), Credential("wrong-fixture-credential")} {
		if _, err = a.Authorize(c, "a", true); err == nil {
			t.Fatal("claimed principal accepted")
		}
	}
	if _, err = a.Authorize(credential, "b", false); err == nil {
		t.Fatal("cross-workspace access accepted")
	}
	if strings.Contains(fmt.Sprintf("%v %#v", credential, credential), string(credential)) {
		t.Fatal("credential leaked in formatting")
	}
	if _, err = json.Marshal(credential); err == nil {
		t.Fatal("credential export accepted")
	}
	readOnly, err := NewAccess(map[Credential][]Membership{credential: {{Principal: "alice", Workspace: "a", Read: true}}})
	if err != nil {
		t.Fatal(err)
	}
	if _, err = readOnly.Authorize(credential, "a", true); err == nil {
		t.Fatal("read permission escalated to install")
	}
}
