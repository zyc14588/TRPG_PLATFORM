// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package httpapi

import (
	"net/http"
	"testing"
)

func TestPresentationExactRoute(t *testing.T) {
	w, id, owned, valid := playerPresentationRoute(http.MethodGet, "/api/v1/workspaces/workspace/games/configuration/presentation")
	if !owned || !valid || w != "workspace" || id != "configuration" {
		t.Fatal("exact route not recognized")
	}
	for _, v := range []struct{ method, path string }{{"POST", "/api/v1/workspaces/w/games/g/presentation"}, {"HEAD", "/api/v1/workspaces/w/games/g/presentation"}, {"GET", "/api/v1/workspaces/w/games/g/presentation/"}, {"GET", "/api/v1/workspaces/w/games/g/presentation/extra"}} {
		if _, _, _, ok := playerPresentationRoute(v.method, v.path); ok {
			t.Fatal("non-exact route accepted")
		}
	}
	for _, path := range []string{"/api/v1/workspaces/w/games", "/api/v1/workspaces/w/rooms/r/preparation", "/api/v1/login"} {
		if _, _, owned, _ := playerPresentationRoute("GET", path); owned {
			t.Fatal("accepted original route intercepted")
		}
	}
}
