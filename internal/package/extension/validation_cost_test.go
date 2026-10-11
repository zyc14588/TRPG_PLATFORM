// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package extension

import (
	"fmt"
	"math"
	"reflect"
	"slices"
	"strings"
	"sync"
	"testing"

	"github.com/santhosh-tekuri/jsonschema/v6"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/jsondocument"
)

func costTestValue(t *testing.T, text string) jsondocument.Value {
	t.Helper()
	value, err := jsondocument.Parse([]byte(text))
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func costTestDocument(t *testing.T, body string) Document {
	t.Helper()
	data := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema",` + body + `}`)
	audit, err := auditSchema(costTestValue(t, string(data)))
	if err != nil {
		t.Fatal(err)
	}
	compiled, err := compileSchema(data)
	if err != nil {
		t.Fatal(err)
	}
	plan := newValidationCostPlan(compiled, audit)
	if plan == nil {
		t.Fatalf("audited core schema has no cost plan: %s", data)
	}
	return Document{Status: Supported, compiled: compiled, audit: audit, costPlan: plan}
}

func TestValidationCostIndependentCounterBoundaries(t *testing.T) {
	// These tests exercise the same overflow-safe primitive used by every
	// operation, including the independent index/preflight budget.
	classes := []struct {
		name  string
		limit int
		field func(*validationCostMeter) *int
	}{
		{"index steps", MaxSchemaValidationSteps, func(m *validationCostMeter) *int { return &m.indexSteps }},
		{"index work", MaxSchemaValidationWork, func(m *validationCostMeter) *int { return &m.indexWork }},
		{"application steps", MaxSchemaValidationSteps, func(m *validationCostMeter) *int { return &m.evaluationSteps }},
		{"application work", MaxSchemaValidationWork, func(m *validationCostMeter) *int { return &m.evaluationWork }},
		{"pattern steps", MaxSchemaValidationSteps, func(m *validationCostMeter) *int { return &m.patternSteps }},
		{"pattern work", MaxSchemaValidationWork, func(m *validationCostMeter) *int { return &m.patternWork }},
		{"unique steps", MaxSchemaValidationSteps, func(m *validationCostMeter) *int { return &m.uniqueSteps }},
		{"unique work", MaxSchemaValidationWork, func(m *validationCostMeter) *int { return &m.uniqueWork }},
		{"assertion steps", MaxSchemaValidationSteps, func(m *validationCostMeter) *int { return &m.assertionSteps }},
		{"assertion work", MaxSchemaValidationWork, func(m *validationCostMeter) *int { return &m.assertionWork }},
		{"regexp work", MaxSchemaValidationWork, func(m *validationCostMeter) *int { return &m.regexpWork }},
	}
	for _, class := range classes {
		t.Run(class.name, func(t *testing.T) {
			for _, amount := range []int{class.limit - 1, class.limit, class.limit + 1} {
				var meter validationCostMeter
				err := costCharge(class.field(&meter), amount, class.limit, class.name)
				if (err != nil) != (amount > class.limit) {
					t.Fatalf("amount=%d error=%v", amount, err)
				}
				if err != nil && *class.field(&meter) != 0 {
					t.Fatal("rejected charge mutated the counter")
				}
			}
			var meter validationCostMeter
			if err := costCharge(class.field(&meter), class.limit-1, class.limit, class.name); err != nil {
				t.Fatal(err)
			}
			if err := costChargeProduct(class.field(&meter), 1, 1, class.limit, class.name); err != nil {
				t.Fatal(err)
			}
			if err := costChargeProduct(class.field(&meter), math.MaxInt, 2, class.limit, class.name); err == nil {
				t.Fatal("multiplication overflow was accepted")
			}
			if err := costCharge(class.field(&meter), math.MaxInt, class.limit, class.name); err == nil {
				t.Fatal("addition overflow was accepted")
			}
			if *class.field(&meter) != class.limit {
				t.Fatal("rejected addition changed the accepted boundary")
			}
		})
	}
	for _, pair := range [][2]int{{0, math.MaxInt}, {math.MaxInt, 0}} {
		if result, ok := costProduct(pair[0], pair[1], MaxSchemaValidationWork); !ok || result != 0 {
			t.Fatalf("zero product=%d,%v", result, ok)
		}
	}
	for _, pair := range [][2]int{{-1, 1}, {1, -1}, {math.MaxInt, math.MaxInt}} {
		if _, ok := costProduct(pair[0], pair[1], MaxSchemaValidationWork); ok {
			t.Fatalf("invalid product accepted: %v", pair)
		}
	}
}

func TestValidationCostInstanceIndexCanonicalSizes(t *testing.T) {
	for _, raw := range []string{
		`null`, `true`, `false`, `""`, `[]`, `{}`,
		`{"~//":"\u0000\n\t\"\\你好","items":[false,0,-0.00,1.20e2,1e-125,1e127]}`,
		`[1.2300e2,-123.456e-7,0.0000100,1000.00,-1e126]`,
	} {
		value := costTestValue(t, raw)
		var meter validationCostMeter
		instance, err := meter.index(value, 0)
		if err != nil {
			t.Fatal(err)
		}
		if instance.nodes != value.Complexity().Nodes || meter.indexSteps != instance.nodes {
			t.Fatalf("one-pass real node count=%d/%d, expected %d", instance.nodes, meter.indexSteps, value.Complexity().Nodes)
		}
		if instance.bytes != meter.indexWork || instance.bytes < len(value.Canonical()) {
			t.Fatalf("canonical work bound=%d/%d, canonical=%d, raw=%s", instance.bytes, meter.indexWork, len(value.Canonical()), raw)
		}
	}
	for _, token := range []string{"0", "-0", "0e308", "1", "-1", "1e127", "1e-125", "12.3400", "1.23e-7", "-123.456e12", "1000.00", "0.0000100"} {
		value := costTestValue(t, token)
		if got, want := costNumberBytes(token), len(value.Canonical()); got != want {
			t.Fatalf("number=%s bytes=%d want=%d", token, got, want)
		}
	}
}

func TestValidationCostApplicatorApplications(t *testing.T) {
	for _, test := range []struct {
		name, schema, payload string
		steps                 int
	}{
		{"present properties", `"properties":{"a":true,"absent":{"allOf":[true,true]}}`, `{"a":1}`, 3},
		{"shared refs per position", `"$defs":{"leaf":true},"properties":{"a":{"$ref":"#/$defs/leaf"},"b":{"$ref":"#/$defs/leaf"}}`, `{"a":1,"b":2}`, 7},
		{"same instance repeated refs", `"$defs":{"leaf":true},"allOf":[{"$ref":"#/$defs/leaf"},{"$ref":"#/$defs/leaf"}]`, `1`, 5},
		{"allOf failures charged", `"allOf":[false,false,true]`, `1`, 4},
		{"anyOf all possible annotations", `"anyOf":[true,false,true]`, `1`, 4},
		{"oneOf second success", `"oneOf":[true,true,false]`, `1`, 4},
		{"if true both branches", `"if":{"type":"integer"},"then":true,"else":false`, `1`, 4},
		{"if false both branches", `"if":{"type":"string"},"then":false,"else":true`, `1`, 4},
		{"compiler removes impossible literal branch", `"if":true,"then":true,"else":false`, `1`, 3},
		{"not", `"not":false`, `1`, 2},
		{"dependency absent", `"dependentSchemas":{"trigger":{"allOf":[true,true]}}`, `{}`, 2},
		{"dependency present", `"dependentSchemas":{"trigger":{"allOf":[true,true]}}`, `{"trigger":1}`, 9},
		{"prefix items contains unevaluated", `"prefixItems":[true],"items":true,"contains":false,"unevaluatedItems":false`, `[1,2,3]`, 13},
		{"object names and unevaluated", `"propertyNames":true,"unevaluatedProperties":false`, `{"a":1,"b":2}`, 7},
	} {
		t.Run(test.name, func(t *testing.T) {
			document := costTestDocument(t, test.schema)
			meter, err := document.costPlan.measure(costTestValue(t, test.payload))
			if err != nil || meter.evaluationSteps != test.steps {
				t.Fatalf("application steps=%d want=%d error=%v", meter.evaluationSteps, test.steps, err)
			}
		})
	}
}

func TestValidationCostPatternCoverageAndInputLocations(t *testing.T) {
	document := costTestDocument(t, `"properties":{"xy":true},"patternProperties":{"^x":true,"y$":true,"^never$":true},"additionalProperties":{"allOf":[true,true]},"propertyNames":{"pattern":"^[a-z]+$"}`)
	meter, err := document.costPlan.measure(costTestValue(t, `{"xy":1,"z":2}`))
	if err != nil {
		t.Fatal(err)
	}
	// root(3) + xy properties/2 patterns(3) + z additional/2 allOf(3)
	// + propertyNames(2). A covered member must not get additionalProperties.
	if meter.evaluationSteps != 11 || meter.patternSteps != 6 || meter.patternWork != 15 {
		t.Fatalf("coverage meter=%+v", meter)
	}
	root := document.costPlan.nodes[document.compiled]
	wantRegexp := 0
	for _, pattern := range root.patterns {
		wantRegexp += pattern.weight * 2 * (3 + 2)
	}
	wantRegexp += document.costPlan.nodes[document.compiled.PropertyNames].pattern * (3 + 2)
	if meter.regexpWork != wantRegexp {
		t.Fatalf("regexp charged=%d expected preflight+executor and virtual-key work=%d", meter.regexpWork, wantRegexp)
	}
	for _, name := range []string{"", "名字/~", strings.Repeat("k", 8192)} {
		payload := fmt.Sprintf("{%q:null}", name)
		meter, err := document.costPlan.measure(costTestValue(t, payload))
		if err != nil {
			t.Fatal(err)
		}
		want := document.costPlan.nodes[document.compiled.PropertyNames].pattern * (1 + len(name))
		for _, pattern := range root.patterns {
			want += pattern.weight * 2 * (1 + len(name))
		}
		if meter.regexpWork != want {
			t.Fatalf("UTF8/long/nonmatching key charged=%d want=%d", meter.regexpWork, want)
		}
	}
}

func TestValidationCostUniqueAlgorithmBoundaryAndLocality(t *testing.T) {
	document := costTestDocument(t, `"properties":{"local":{"uniqueItems":true},"other":true}`)
	for _, width := range []int{0, 1, 20, 21} {
		items := make([]string, width)
		for i := range items {
			items[i] = fmt.Sprintf(`{"nested":[%d,1e0],"common-prefix":%q}`, i, strings.Repeat("x", 128)+fmt.Sprint(i))
		}
		value := costTestValue(t, `{"local":[`+strings.Join(items, ",")+`],"other":"`+strings.Repeat("z", 10000)+`"}`)
		meter, err := document.costPlan.measure(value)
		if err != nil {
			t.Fatal(err)
		}
		var index validationCostMeter
		instance, err := index.index(value, 0)
		if err != nil {
			t.Fatal(err)
		}
		array := instance.lookup["local"]
		wantWork, wantSteps := 0, 0
		if width > 1 {
			wantWork, wantSteps = width*array.bytes, width*array.nodes
			if width > 20 {
				wantWork += array.bytes + array.sortWork
			}
		}
		if meter.uniqueWork != wantWork || meter.uniqueSteps != wantSteps {
			t.Fatalf("width=%d unique=%d/%d want local bytes/nodes=%d/%d", width, meter.uniqueWork, meter.uniqueSteps, wantWork, wantSteps)
		}
	}
	falseDocument := costTestDocument(t, `"uniqueItems":false`)
	meter, err := falseDocument.costPlan.measure(costTestValue(t, `[1,1,1.0]`))
	if err != nil || meter.uniqueSteps != 0 || meter.uniqueWork != 0 {
		t.Fatalf("false uniqueness charged=%+v error=%v", meter, err)
	}
}

func TestValidationCostAssertionLocality(t *testing.T) {
	document := costTestDocument(t, `"required":["trigger"],"dependentRequired":{"trigger":["local"]},"properties":{"local":{"enum":[{"a":[1,2]},"`+strings.Repeat("x", 512)+`"],"const":{"a":[1,2]}},"absent":{"const":"`+strings.Repeat("y", 2048)+`"}}`)
	value := costTestValue(t, `{"trigger":true,"local":{"a":[1,2]},"other":"`+strings.Repeat("z", 65536)+`"}`)
	meter, err := document.costPlan.measure(value)
	if err != nil {
		t.Fatal(err)
	}
	var index validationCostMeter
	instance, err := index.index(value, 0)
	if err != nil {
		t.Fatal(err)
	}
	root := document.costPlan.nodes[document.compiled].assertions
	local := document.costPlan.nodes[document.compiled.Properties["local"]].assertions
	want := root.dataBytes + root.comparisons*instance.bytes + local.dataBytes + local.comparisons*instance.lookup["local"].bytes
	if meter.assertionWork != want || meter.assertionSteps != root.steps+local.steps {
		t.Fatalf("assertion work=%+v expected=%d; absent/sibling assertions must not apply", meter, want)
	}
}

func TestValidationCostUnsupportedCompiledStructureFallsBack(t *testing.T) {
	document := costTestDocument(t, `"type":"integer"`)
	for _, change := range []func(*jsonschema.Schema){
		func(s *jsonschema.Schema) { s.DraftVersion = 7 },
		func(s *jsonschema.Schema) { s.Location += "/uncovered" },
		func(s *jsonschema.Schema) { s.Format = &jsonschema.Format{} },
		func(s *jsonschema.Schema) { s.ContentEncoding = &jsonschema.Decoder{} },
		func(s *jsonschema.Schema) { s.ContentMediaType = &jsonschema.MediaType{} },
		func(s *jsonschema.Schema) { s.ContentSchema = s },
		func(s *jsonschema.Schema) { s.RecursiveRef = s },
		func(s *jsonschema.Schema) { s.DynamicRef = &jsonschema.DynamicRef{} },
		func(s *jsonschema.Schema) { s.Items = s },
		func(s *jsonschema.Schema) { s.AdditionalItems = false },
		func(s *jsonschema.Schema) { s.AdditionalProperties = "uncovered" },
		func(s *jsonschema.Schema) { s.Dependencies = map[string]any{"a": s} },
		func(s *jsonschema.Schema) { s.Ref = s },
	} {
		// Explicit unit-only compiled fixtures. Production construction still
		// uses exactly the compiler plus the immutable audited source metadata.
		modified := *document.compiled
		change(&modified)
		if newValidationCostPlan(&modified, document.audit) != nil {
			t.Fatalf("uncovered compiled structure did not select the legacy audit: %+v", modified)
		}
	}
	document.costPlan = nil
	for _, test := range []struct {
		value    string
		rejected bool
	}{{"1", false}, {`"wrong"`, true}} {
		value := costTestValue(t, test.value)
		if err := document.audit.validatePayloadShape(value); err != nil {
			t.Fatal(err)
		}
		err := document.validateValue(value, len(value.Canonical()))
		if IsCode(err, ErrSchemaValidation) != test.rejected {
			t.Fatalf("fallback schema semantics=%v", err)
		}
	}
	if err := document.audit.validatePayloadBytes(MaxSchemaValidationWork+1, 0); err == nil {
		t.Fatal("legacy fallback lost its byte guard")
	}
}

func TestValidationCostDifferentialPinnedValidator(t *testing.T) {
	corpus := []struct {
		schema   string
		payloads []string
	}{
		{`"type":"integer","minimum":-1,"maximum":3,"multipleOf":0.5`, []string{`1e0`, `-1`, `3.0`, `3.5`, `"1"`}},
		{`"const":{"a":[1,2.0],"b":true}`, []string{`{"b":true,"a":[1.0,2]}`, `{"b":true,"a":[1,3]}`, `null`}},
		{`"enum":[{"a":[1,2]},"` + strings.Repeat("x", 100) + `z",null]`, []string{`{"a":[1e0,2]}`, `"` + strings.Repeat("x", 100) + `y"`, `false`, `null`}},
		{`"type":"object","properties":{"a":{"type":"integer"}},"patternProperties":{"^a":{"minimum":2},"a$":{"maximum":5}},"additionalProperties":false,"required":["a"]`, []string{`{"a":3}`, `{"a":1}`, `{"a":6}`, `{"a":3,"b":1}`, `{}`}},
		{`"propertyNames":{"pattern":"^[\\p{L}]+$","minLength":1,"maxLength":2},"minProperties":1,"maxProperties":2`, []string{`{"名字":1}`, `{"é":true,"a":1}`, `{"":null}`, `{"a/b":1}`, `{}`}},
		{`"dependentSchemas":{"a":{"required":["b"]}},"dependentRequired":{"b":["c"]}`, []string{`{}`, `{"a":1}`, `{"a":1,"b":2,"c":3}`, `{"b":2}`}},
		{`"anyOf":[{"properties":{"a":true},"required":["a"]},{"properties":{"b":true},"required":["b"]}],"unevaluatedProperties":false`, []string{`{"a":1}`, `{"a":1,"b":2}`, `{"a":1,"c":2}`, `{}`}},
		{`"oneOf":[{"type":"number"},{"type":"integer"}]`, []string{`1`, `1.5`, `null`}},
		{`"if":{"type":"integer"},"then":{"minimum":0},"else":{"type":"string"},"not":{"const":"forbidden"}`, []string{`1`, `-1`, `"allowed"`, `"forbidden"`, `null`}},
		{`"prefixItems":[{"type":"integer"}],"items":{"type":"string"},"contains":{"const":"x"},"minContains":1,"maxContains":2`, []string{`[1,"x"]`, `[1,"x","x"]`, `[1,"x","x","x"]`, `[1,"y"]`, `["bad","x"]`}},
		{`"prefixItems":[true],"contains":{"type":"integer"},"unevaluatedItems":false`, []string{`[null,1,2]`, `[null,"a"]`, `[]`}},
		{`"uniqueItems":true`, []string{`[1,2,3]`, `[1,1.0]`, `[0,-0]`, `[{"a":[1,2]}, {"a":[1.0,2]}]`, `[[1,2],[2,1]]`}},
		{`"$defs":{"名字/~":{"$anchor":"leaf","type":"string","pattern":"^a+$"}},"properties":{"名字/~":{"$ref":"#leaf"}},"additionalProperties":{"$ref":"#/$defs/%E5%90%8D%E5%AD%97~1~0"}`, []string{`{"名字/~":"aa","other":"a"}`, `{"名字/~":"b"}`}},
		{`"type":"string","minLength":2,"maxLength":3,"pattern":"\\p{L}+"`, []string{`"你好"`, `"éa"`, `"a"`, `"abcd"`, `"12"`}},
	}
	for index, test := range corpus {
		t.Run(fmt.Sprint(index), func(t *testing.T) {
			document := costTestDocument(t, test.schema)
			for _, raw := range test.payloads {
				value := costTestValue(t, raw)
				oracle := document.compiled.Validate(value.Interface())
				for _, local := range []bool{true, false} {
					candidate := document
					if !local {
						candidate.costPlan = nil
						if err := candidate.audit.validatePayloadShape(value); err != nil {
							t.Fatal(err)
						}
					}
					actual := candidate.validateValue(value, len(value.Canonical()))
					if IsCode(actual, ErrInvalid) || (actual != nil) != (oracle != nil) || (actual != nil && !IsCode(actual, ErrSchemaValidation)) {
						t.Fatalf("local=%v value=%s oracle=%v candidate=%v", local, raw, oracle, actual)
					}
				}
			}
		})
	}
}

func TestValidationCostFailureResetAndConcurrentDocument(t *testing.T) {
	document := costTestDocument(t, `"properties":{"tag":{"type":"string","pattern":"^(?:a|A){1000}b$"},"other":true}`)
	good := []byte(`{"tag":"` + strings.Repeat("a", 1000) + `b","other":"ok"}`)
	tooExpensive := []byte(`{"tag":"` + strings.Repeat("a", 16384) + `b"}`)
	before := make(map[*jsonschema.Schema]validationCostNode)
	for schema, node := range document.costPlan.nodes {
		before[schema] = node
	}
	var wait sync.WaitGroup
	for worker := 0; worker < 8; worker++ {
		wait.Add(1)
		go func() {
			defer wait.Done()
			for repeat := 0; repeat < 8; repeat++ {
				if _, err := document.ValidateReplacement(tooExpensive); !IsCode(err, ErrInvalid) {
					t.Errorf("resource rejection=%v", err)
				}
				if _, err := document.ValidateReplacement([]byte(`{"tag":"wrong"}`)); !IsCode(err, ErrSchemaValidation) {
					t.Errorf("schema rejection=%v", err)
				}
				if _, err := document.ValidateReplacement(good); err != nil {
					t.Errorf("validation after rejection=%v", err)
				}
			}
		}()
	}
	wait.Wait()
	if !reflect.DeepEqual(before, document.costPlan.nodes) {
		t.Fatal("validation mutated the shared compiled cost plan")
	}
}

func TestValidationCostResourceFailureIsNotSchemaFalse(t *testing.T) {
	heavy := `{"type":"string","pattern":"^(?:a|A){1000}b$"}`
	for _, schema := range []string{
		`"not":` + heavy,
		`"anyOf":[true,` + heavy + `]`,
		`"oneOf":[true,` + heavy + `]`,
		`"if":{"type":"integer"},"then":true,"else":` + heavy,
		`"if":{"type":"string"},"then":true,"else":` + heavy,
	} {
		document := costTestDocument(t, schema)
		value := costTestValue(t, `"`+strings.Repeat("a", 16384)+`b"`)
		err := document.validateValue(value, len(value.Canonical()))
		if !IsCode(err, ErrInvalid) || !strings.Contains(err.Error(), "regular-expression byte") {
			t.Fatalf("resource failure was treated as schema false: schema=%s error=%v", schema, err)
		}
	}
}

func TestValidationCostActualPreflightNodeBoundary(t *testing.T) {
	document := costTestDocument(t, `"type":"array"`)
	for _, nodes := range []int{MaxSchemaValidationSteps - 1, MaxSchemaValidationSteps, MaxSchemaValidationSteps + 1} {
		value := costTestValue(t, `[`+strings.Repeat(`null,`, nodes-2)+`null]`)
		meter, err := document.costPlan.measure(value)
		if (err != nil) != (nodes > MaxSchemaValidationSteps) {
			t.Fatalf("nodes=%d preflight=%+v error=%v", nodes, meter, err)
		}
		if err == nil && (meter.indexSteps != nodes || meter.evaluationSteps != nodes) {
			t.Fatalf("empty-applicator index/application accounting=%+v", meter)
		}
	}
	// The root plus each items application must reserve failed error-tree work
	// before Validate constructs it, even though indexing alone fits the cap.
	document = costTestDocument(t, `"items":{"type":"integer"}`)
	value := costTestValue(t, `[`+strings.Repeat(`null,`, 32999)+`null]`)
	if _, err := document.costPlan.measure(value); err == nil {
		t.Fatal("33,000 failing items bypassed the application/error-tree guard")
	}
}

func TestValidationCostUniqueObjectSortingPrefixBound(t *testing.T) {
	for _, width := range []int{1, 2, 20, 21, 64, 257, 1024} {
		for _, order := range []string{"sorted", "reversed", "organ-pipe", "alternating"} {
			keys := make([]string, width)
			members := make([]string, width)
			for i := range keys {
				n := i
				switch order {
				case "reversed":
					n = width - 1 - i
				case "organ-pipe":
					if i >= (width+1)/2 {
						n = width - 1 - i
					}
				case "alternating":
					if i%2 == 0 {
						n = i / 2
					} else {
						n = width - 1 - i/2
					}
				}
				// Keep the input unique even for the organ-pipe ordering.
				keys[i] = strings.Repeat("shared-prefix", 16) + fmt.Sprintf("%04d-%04d", n, i)
				members[i] = fmt.Sprintf("%q:null", keys[i])
			}
			var meter validationCostMeter
			instance, err := meter.index(costTestValue(t, `{`+strings.Join(members, ",")+`}`), 0)
			if err != nil {
				t.Fatal(err)
			}
			comparisonBytes := 0
			// The pinned validator uses the same standard slices sorting family
			// for object keys. Instrument comparison prefix traffic separately
			// from the implementation's static upper-bound calculation.
			slices.SortFunc(keys, func(a, b string) int {
				common := 0
				for common < min(len(a), len(b)) && a[common] == b[common] {
					common++
				}
				comparisonBytes += common + 1
				return strings.Compare(a, b)
			})
			if comparisonBytes > instance.sortWork {
				t.Fatalf("width=%d order=%s actual prefix work=%d reserved=%d", width, order, comparisonBytes, instance.sortWork)
			}
		}
	}
}
