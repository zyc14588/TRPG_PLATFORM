// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package migration_test

import (
	"bytes"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
	"github.com/zyc14588/TRPG_PLATFORM/tests/fuzz/support"
)

// Only bounded, data-only plans are varied. The source/target schemas and
// snapshot are trusted fixed inputs; no generated Session state machine exists.
func FuzzMigrationEntrypoint(f *testing.F) {
	before, target, valid := support.Migration()
	f.Add(support.JSON(valid))
	bad := valid
	bad.StateFields = []migration.FieldMove{{From: "counter", To: "cap:sql"}}
	f.Add(support.JSON(bad))
	f.Fuzz(func(t *testing.T, raw []byte) {
		if len(raw) > support.MaxInput {
			return
		}
		var p migration.Plan
		if checkpoint.StrictDecode(raw, &p, support.MaxInput) != nil {
			return
		}
		original := support.JSON(before)
		a, err := p.Apply(before, target)
		if !bytes.Equal(original, support.JSON(before)) {
			t.Fatal("migration changed source snapshot")
		}
		if err != nil {
			return
		}
		if a.Binding != target.Binding || a.Version != before.Version+1 || target.ValidateSnapshot(a) != nil {
			t.Fatal("accepted plan produced invalid complete target")
		}
		b, err := p.Apply(before, target)
		if err != nil || !reflect.DeepEqual(a, b) {
			t.Fatal("data-only plan is nondeterministic")
		}
	})
}
