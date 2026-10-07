// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"strings"
	"testing"
)

func TestSessionAccessCannotBeManufacturedOrUseAnotherService(t *testing.T) {
	s := &Service{}
	resolver := func(context.Context) (command.NativeSeat, error) { return command.NativeSeat{}, nil }
	if _, e := s.ConnectSession(context.Background(), SessionAccess{}, resolver); e != auth.ErrDenied {
		t.Fatal("zero access issued transport")
	}
	foreign := &sessionAccessData{issuer: &Service{}}
	if _, e := s.ConnectSession(context.Background(), SessionAccess{data: &foreign}, resolver); e != auth.ErrDenied {
		t.Fatal("foreign access accepted")
	}
}
func TestSessionAccessAndTransportOpaqueFormatting(t *testing.T) {
	marker := "private_bridge_marker"
	d := &sessionAccessData{value: SessionAccessData{Principal: marker, Seat: marker}}
	access := SessionAccess{data: &d}
	for _, v := range []any{access, &access, &SessionTransport{}, SessionTransport{}} {
		for _, verb := range []string{"%v", "%+v", "%#v", "%d", "%f", "%w", "%*v"} {
			if strings.Contains(fmt.Sprintf(verb, v), marker) {
				t.Fatal("private bridge access escaped formatting")
			}
		}
		if _, e := json.Marshal(v); e == nil {
			t.Fatal("private bridge JSON export permitted")
		}
	}
}
