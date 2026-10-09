// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package httpapi

import (
	"net/http"
	"testing"
)

func TestRoomPresentationExactRouteAndExistingDelegation(t *testing.T) {
	w, room, owned, valid := playerPresentationRoute(http.MethodGet, "/api/v1/workspaces/workspace/rooms/room/presentation")
	if !owned || !valid || w != "workspace" || room != "room" {
		t.Fatal("current-room read route not recognized")
	}
	for _, method := range []string{http.MethodPost, http.MethodHead, http.MethodPut, http.MethodDelete} {
		if _, _, owned, valid := playerPresentationRoute(method, "/api/v1/workspaces/w/rooms/r/presentation"); !owned || valid {
			t.Fatal("room presentation accepted a mutation or delegated its invalid method")
		}
	}
	for _, suffix := range []string{"/", "/extra"} {
		if _, _, owned, valid := playerPresentationRoute(http.MethodGet, "/api/v1/workspaces/w/rooms/r/presentation"+suffix); !owned || valid {
			t.Fatal("non-exact room presentation was accepted")
		}
	}
	for _, path := range []string{"/api/v1/workspaces/w/games", "/api/v1/workspaces/w/rooms/r", "/api/v1/workspaces/w/rooms/r/preparation", "/api/v1/workspaces/w/rooms/r/consent", "/api/v1/workspaces/w/rooms/r/session/snapshot", "/api/v1/auth/context"} {
		if _, _, owned, _ := playerPresentationRoute(http.MethodGet, path); owned {
			t.Fatal("existing production route intercepted")
		}
	}
}
