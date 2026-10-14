// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package command

import (
	"context"
	"fmt"
	"reflect"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

func TestViewPolicyRejectsMalformedSelectors(t *testing.T) {
	scalar := &ViewSelector{Kind: "scalar"}
	cycle := &ViewSelector{Kind: "table"}
	cycle.Fields = map[string]*ViewSelector{"loop": cycle}
	indirect := &ViewSelector{Kind: "table"}
	indirect.Fields = map[string]*ViewSelector{"loop": {Kind: "array", Element: indirect}}
	for name, selector := range map[string]*ViewSelector{
		"unknown-root":      {Kind: "unknown"},
		"scalar-root":       scalar,
		"array-root":        {Kind: "array", Element: scalar},
		"nullable-root":     {Kind: "table", Nullable: true},
		"table-element":     {Kind: "table", Element: scalar},
		"nil-child":         {Kind: "table", Fields: map[string]*ViewSelector{"child": nil}},
		"scalar-fields":     {Kind: "table", Fields: map[string]*ViewSelector{"child": {Kind: "scalar", Fields: map[string]*ViewSelector{}}}},
		"scalar-element":    {Kind: "table", Fields: map[string]*ViewSelector{"child": {Kind: "scalar", Element: scalar}}},
		"array-fields":      {Kind: "table", Fields: map[string]*ViewSelector{"child": {Kind: "array", Fields: map[string]*ViewSelector{}, Element: scalar}}},
		"array-no-element":  {Kind: "table", Fields: map[string]*ViewSelector{"child": {Kind: "array"}}},
		"unknown-child":     {Kind: "table", Fields: map[string]*ViewSelector{"child": {Kind: "unknown"}}},
		"literal-wildcard":  {Kind: "table", Fields: map[string]*ViewSelector{"*": scalar}},
		"empty-field":       {Kind: "table", Fields: map[string]*ViewSelector{"": scalar}},
		"path-field":        {Kind: "table", Fields: map[string]*ViewSelector{"a/b": scalar}},
		"oversize-field":    {Kind: "table", Fields: map[string]*ViewSelector{strings.Repeat("a", 129): scalar}},
		"direct-cycle":      cycle,
		"array-table-cycle": indirect,
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := OwnPolicy(ViewPolicy{ViewProjection: selector}); err != ErrDenied {
				t.Fatal("malformed selector accepted", err)
			}
		})
	}
	if _, err := OwnPolicy(ViewPolicy{ViewFields: []string{"same"}, ViewProjection: &ViewSelector{Kind: "table", Fields: map[string]*ViewSelector{"same": scalar}}}); err != ErrDenied {
		t.Fatal("legacy and selector grants overlapped")
	}
}

func TestViewPolicyCountsEveryReferenceAndRetainsSeparateBudgets(t *testing.T) {
	fields := map[string]*ViewSelector{}
	shared := &ViewSelector{Kind: "scalar"}
	for n := 0; n < 127; n++ {
		fields[fmt.Sprintf("field-%d", n)] = shared
	}
	p := ViewPolicy{ViewProjection: &ViewSelector{Kind: "table", Fields: fields}}
	if _, err := OwnPolicy(p); err != nil {
		t.Fatal("128 referenced nodes denied", err)
	}
	fields["one-more"] = shared
	if _, err := OwnPolicy(p); err != ErrDenied {
		t.Fatal("repeated aliases bypassed the 128-node budget")
	}
	delete(fields, "one-more")
	p.ViewFields = []string{"legacy"}
	if _, err := OwnPolicy(p); err != ErrDenied {
		t.Fatal("legacy fields not charged against the tree budget")
	}
	delete(fields, "field-0")
	p.ResultFields = make([]string, 128)
	p.EventFields = map[string][]string{"event": make([]string, 128)}
	for n := range p.ResultFields {
		p.ResultFields[n] = fmt.Sprintf("result-%d", n)
		p.EventFields["event"][n] = fmt.Sprintf("event-%d", n)
	}
	if _, err := OwnPolicy(p); err != nil {
		t.Fatal("existing result/event budgets changed", err)
	}
	p.ResultFields = append(p.ResultFields, "one-more")
	if _, err := OwnPolicy(p); err != ErrDenied {
		t.Fatal("result budget increased")
	}
	legacy := ViewPolicy{ViewFields: make([]string, 128)}
	for n := range legacy.ViewFields {
		legacy.ViewFields[n] = fmt.Sprintf("legacy-%d", n)
	}
	if _, err := OwnPolicy(legacy); err != nil {
		t.Fatal("legacy cap changed", err)
	}
	legacy.ViewFields = append(legacy.ViewFields, "one-more")
	if _, err := OwnPolicy(legacy); err != ErrDenied {
		t.Fatal("legacy cap increased")
	}
}

func TestViewPolicyDepthUsesCheckpointBoundary(t *testing.T) {
	for _, depth := range []int{checkpoint.MaxDepth, checkpoint.MaxDepth + 1} {
		node := &ViewSelector{Kind: "scalar"}
		for n := 0; n < depth; n++ {
			node = &ViewSelector{Kind: "table", Fields: map[string]*ViewSelector{"child": node}}
		}
		_, err := OwnPolicy(ViewPolicy{ViewProjection: node})
		if (err == nil) != (depth == checkpoint.MaxDepth) {
			t.Fatal("selector depth boundary changed", depth, err)
		}
	}
}

func TestViewPolicyOwnsAllSelectorAliasesAndLegacyFields(t *testing.T) {
	child := &ViewSelector{Kind: "table", Fields: map[string]*ViewSelector{"visible": {Kind: "scalar"}}}
	root := &ViewSelector{Kind: "table", Fields: map[string]*ViewSelector{
		"first": child, "second": child, "list": {Kind: "array", Element: child},
	}}
	p := ViewPolicy{ViewFields: []string{"counter"}, ResultFields: []string{"result"}, EventFields: map[string][]string{"event": {"visible"}}, ScalarResult: true, ViewProjection: root}
	q, err := OwnPolicy(p)
	if err != nil {
		t.Fatal(err)
	}
	if q.ViewProjection == root || q.ViewProjection.Fields["first"] == q.ViewProjection.Fields["second"] || q.ViewProjection.Fields["first"] == q.ViewProjection.Fields["list"].Element {
		t.Fatal("selector aliases were retained")
	}
	q.ViewProjection.Fields["first"].Fields["visible"].Nullable = true
	if child.Fields["visible"].Nullable || q.ViewProjection.Fields["second"].Fields["visible"].Nullable || q.ViewProjection.Fields["list"].Element.Fields["visible"].Nullable {
		t.Fatal("mutation crossed independent selector branches")
	}
	delete(root.Fields, "second")
	child.Fields["secret"] = &ViewSelector{Kind: "scalar"}
	p.ViewFields[0], p.ResultFields[0], p.EventFields["event"][0] = "changed", "changed", "changed"
	if _, ok := q.ViewProjection.Fields["second"]; !ok {
		t.Fatal("owned root map changed")
	}
	if _, ok := q.ViewProjection.Fields["list"].Element.Fields["secret"]; ok {
		t.Fatal("owned element map changed")
	}
	if !reflect.DeepEqual(q.ViewFields, []string{"counter"}) || !reflect.DeepEqual(q.ResultFields, []string{"result"}) || !reflect.DeepEqual(q.EventFields["event"], []string{"visible"}) || !q.ScalarResult {
		t.Fatal("legacy ownership or scalar grant changed")
	}
}

func TestAuthorityOwnsAndRevalidatesNestedPolicies(t *testing.T) {
	a, seat, _ := nativeFixture(t)
	seat.Views.ViewProjection = &ViewSelector{Kind: "table", Fields: map[string]*ViewSelector{"details": {Kind: "table", Fields: map[string]*ViewSelector{"visible": {Kind: "scalar"}}}}}
	ctx := context.Background()
	i, err := a.IssueNative(ctx, func(context.Context) (NativeSeat, error) { return seat, nil })
	if err != nil {
		t.Fatal(err)
	}
	defer a.ReleaseNative(i)
	p, err := a.Policy(i)
	if err != nil {
		t.Fatal(err)
	}
	p.ViewProjection.Fields["details"].Fields["visible"].Nullable = true
	again, err := a.Policy(i)
	if err != nil || again.ViewProjection.Fields["details"].Fields["visible"].Nullable {
		t.Fatal("resolver or subsequent policy retained caller mutations")
	}
	delete(seat.Views.ViewProjection.Fields["details"].Fields, "visible")
	again, err = a.Policy(i)
	if err != nil || len(again.ViewProjection.Fields["details"].Fields) != 0 {
		t.Fatal("current nested policy was cached")
	}
	seat.Views.ViewProjection.Fields["details"].Fields["bad"] = nil
	if a.Verify(i) != ErrDenied {
		t.Fatal("invalid current nested policy remained authorized")
	}
}

func TestFixtureAuthorityOwnsNestedPolicyBeforeCallerMutation(t *testing.T) {
	_, seat, _ := nativeFixture(t)
	root := &ViewSelector{Kind: "table", Fields: map[string]*ViewSelector{"details": {Kind: "table", Fields: map[string]*ViewSelector{"visible": {Kind: "scalar"}}}}}
	a, err := NewFixtureAuthority([]FixtureSeat{{Credential: "fixture-nested-0123456789abcdef", Binding: seat.Binding, Principal: seat.Principal, Seat: seat.Seat, Views: ViewPolicy{ViewProjection: root}}})
	if err != nil {
		t.Fatal(err)
	}
	i, err := a.Authenticate("fixture-nested-0123456789abcdef", seat.Binding.Session, seat.Seat)
	if err != nil {
		t.Fatal(err)
	}
	root.Fields["details"].Fields["visible"].Nullable = true
	root.Fields["details"].Fields["secret"] = &ViewSelector{Kind: "scalar"}
	p, err := a.Policy(i)
	if err != nil || len(p.ViewProjection.Fields["details"].Fields) != 1 || p.ViewProjection.Fields["details"].Fields["visible"].Nullable {
		t.Fatal("fixture policy retained caller nodes", err)
	}
	delete(p.ViewProjection.Fields["details"].Fields, "visible")
	p, err = a.Policy(i)
	if err != nil || len(p.ViewProjection.Fields["details"].Fields) != 1 {
		t.Fatal("fixture returned its retained selector map", err)
	}
}
