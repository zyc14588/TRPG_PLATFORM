// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"strings"
	"testing"
)

func TestPreparationDiagnosticsExcludePrivateBoundariesAndCredentials(t *testing.T) {
	marker := "PRIVATE_LAUNCH_BOUNDARY_fcd76e2"
	c, p, a, _, _ := gateFixture()
	c.Request.Credential = store.Credential(marker)
	x := a[0].StorageValue()
	x.Boundaries = []string{marker}
	a[0] = auth.RoomSecret(x)
	d := &serviceData{configs: map[string]Configuration{"private": auth.RoomSecret(c)}}
	s := &Service{data: &d}
	cd := &coordinatorData{entries: map[string]*activation{marker: {}}}
	co := &Coordinator{data: &cd}
	rd := &reservationData{owner: co, entry: &activation{}}
	reservation := &Reservation{data: &rd}
	for _, v := range []any{auth.RoomSecret(c), auth.RoomSecret(p), a[0], s, *s, co, *co, reservation, *reservation} {
		for _, verb := range []string{"%v", "%+v", "%#v", "%d", "%s", "%q", "%x", "%w"} {
			if strings.Contains(fmt.Sprintf(verb, v), marker) {
				t.Fatal("private launch value escaped diagnostic formatting")
			}
		}
		raw, e := json.Marshal(v)
		if e == nil || strings.Contains(string(raw), marker) {
			t.Fatal("private launch value exported as ordinary JSON")
		}
	}
}
