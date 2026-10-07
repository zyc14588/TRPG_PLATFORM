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
