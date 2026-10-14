// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package session

import (
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"strings"
	"testing"
)

func TestNativeServiceRequiresTrustedCompositionAndRedactsHandles(t *testing.T) {
	if _, e := New(Options{}); e != auth.ErrInvalid {
		t.Fatal("default service granted capabilities")
	}
	marker := "private_connection_cookie_marker"
	d := &connectionData{caller: auth.RoomSecret(launch.CallerData{Credential: auth.BrowserCookie(marker), CSRF: marker})}
	c := &Connection{data: &d}
	for _, v := range []any{c, *c, auth.RoomSecret(ExportData{Seat: marker}), auth.RoomSecret(ResultData{CommandID: marker})} {
		for _, verb := range []string{"%v", "%+v", "%#v", "%d", "%f", "%w", "%*v"} {
			if strings.Contains(fmt.Sprintf(verb, v), marker) {
				t.Fatal("native service secret escaped formatting")
			}
		}
		if _, e := json.Marshal(v); e == nil {
			t.Fatal("native service secret JSON export permitted")
		}
	}
	if SafeError(fmt.Errorf("private-driver-marker")) != auth.ErrUnavailable {
		t.Fatal("raw driver diagnostic escaped")
	}
}
func TestExportManagementStillRequiresCurrentRequesterRole(t *testing.T) {
	p := SeatPolicy{Exports: map[ExportKind]command.ViewPolicy{Host: {}, Administrator: {}, Personal: {}}}
	a := launch.SessionAccessData{}
	if _, e := allowedExport(a, p, Host); e != auth.ErrDenied {
		t.Fatal("non-host exported host profile")
	}
	if _, e := allowedExport(a, p, Administrator); e != auth.ErrDenied {
		t.Fatal("non-manager exported administrative profile")
	}
	if _, e := allowedExport(a, p, Public); e != auth.ErrDenied {
		t.Fatal("absent export profile allowed")
	}
	if _, e := allowedExport(a, p, Personal); e != nil {
		t.Fatal("approved personal profile denied")
	}
	q, e := ownPolicy(SeatPolicy{View: command.ViewPolicy{ViewFields: []string{"counter"}}, Exports: map[ExportKind]command.ViewPolicy{Personal: {ViewFields: []string{"counter"}}}})
	if e != nil {
		t.Fatal("own trusted policy")
	}
	q.Exports[Personal].ViewFields[0] = "other"
	if q.View.ViewFields[0] != "counter" {
		t.Fatal("policy filters aliased")
	}
	if _, e := ownPolicy(SeatPolicy{Exports: map[ExportKind]command.ViewPolicy{"unregistered": {}}}); e != auth.ErrDenied {
		t.Fatal("unregistered export profile allowed")
	}
}

func TestServiceOwnsNestedLiveAndExportPoliciesIndependently(t *testing.T) {
	leaf := &command.ViewSelector{Kind: "scalar"}
	record := &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"visible": leaf}}
	root := &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"record": record, "list": {Kind: "array", Element: record}}}
	view := command.ViewPolicy{ViewProjection: root}
	p, err := ownPolicy(SeatPolicy{View: view, Exports: map[ExportKind]command.ViewPolicy{Personal: view, Host: view}})
	if err != nil {
		t.Fatal(err)
	}
	leaf.Nullable = true
	record.Fields["secret"] = &command.ViewSelector{Kind: "scalar"}
	delete(root.Fields, "list")
	for _, owned := range []command.ViewPolicy{p.View, p.Exports[Personal], p.Exports[Host]} {
		if len(owned.ViewProjection.Fields) != 2 || len(owned.ViewProjection.Fields["record"].Fields) != 1 || owned.ViewProjection.Fields["record"].Fields["visible"].Nullable {
			t.Fatal("caller mutation reached an owned service policy")
		}
	}
	p.View.ViewProjection.Fields["record"].Fields["visible"].Nullable = true
	p.Exports[Personal].ViewProjection.Fields["list"].Element.Fields["visible"].Nullable = true
	if p.View.ViewProjection.Fields["list"].Element.Fields["visible"].Nullable || p.Exports[Personal].ViewProjection.Fields["record"].Fields["visible"].Nullable || p.Exports[Host].ViewProjection.Fields["list"].Element.Fields["visible"].Nullable {
		t.Fatal("live, export or element policy ownership was shared")
	}
	if _, err = allowedExport(launch.SessionAccessData{}, p, Host); err != auth.ErrDenied {
		t.Fatal("nested host export bypassed role gate")
	}
	if _, err = allowedExport(launch.SessionAccessData{}, p, Personal); err != nil {
		t.Fatal("approved nested personal export denied")
	}
	malformed := command.ViewPolicy{ViewProjection: &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"child": nil}}}
	for _, policy := range []SeatPolicy{{View: malformed}, {Exports: map[ExportKind]command.ViewPolicy{Personal: malformed}}} {
		if _, err := ownPolicy(policy); err != auth.ErrDenied {
			t.Fatal("invalid live or export selector accepted")
		}
	}
}
