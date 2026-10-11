// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package extension

import (
	"fmt"
	"net/url"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/santhosh-tekuri/jsonschema/v6"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/jsondocument"
)

// validationCostPlan contains only immutable compiler/audit metadata. A nil
// plan means that an execution structure is not covered, so the original
// whole-document audit must run. No request state lives in compiled schemas
// or shared regexps. This traversal estimates work, never schema truth.
type validationCostPlan struct {
	root  *jsonschema.Schema
	nodes map[*jsonschema.Schema]validationCostNode
}

type validationCostNode struct {
	assertions assertionCost
	pattern    int
	patterns   []validationCostPattern
	scanSteps  int
	scanBytes  int
}

type validationCostPattern struct {
	regexp *regexp.Regexp
	schema *jsonschema.Schema
	weight int
}

func newValidationCostPlan(root *jsonschema.Schema, audit schemaAudit) *validationCostPlan {
	if root == nil {
		return nil
	}
	plan := &validationCostPlan{root: root, nodes: make(map[*jsonschema.Schema]validationCostNode)}
	visiting := make(map[*jsonschema.Schema]bool)
	var visit func(*jsonschema.Schema) bool
	visit = func(schema *jsonschema.Schema) bool {
		if schema == nil {
			return true
		}
		if visiting[schema] {
			return false
		}
		if _, exists := plan.nodes[schema]; exists {
			return true
		}
		if len(plan.nodes)+len(visiting) >= MaxSchemaNodes || schema.DraftVersion != 2020 || schema.RecursiveRef != nil || schema.DynamicRef != nil || schema.Format != nil || schema.ContentEncoding != nil || schema.ContentMediaType != nil || schema.ContentSchema != nil || len(schema.Extensions) != 0 || len(schema.Dependencies) != 0 || schema.Items != nil || schema.AdditionalItems != nil {
			return false
		}
		_, fragment, _ := strings.Cut(schema.Location, "#")
		fragment, err := url.PathUnescape(fragment)
		if err != nil {
			return false
		}
		location := "#" + fragment
		if _, covered := audit.locations[location]; !covered {
			return false
		}
		node := validationCostNode{assertions: audit.localAssertions[location]}
		programs := audit.localRegexpPrograms[location]
		if schema.Pattern != nil {
			if _, ok := schema.Pattern.(*regexp.Regexp); !ok {
				return false
			}
			node.pattern = programs[schema.Pattern.String()]
			if node.pattern <= 0 {
				return false
			}
		}
		children := []*jsonschema.Schema{schema.Ref, schema.Not, schema.If, schema.Then, schema.Else, schema.PropertyNames, schema.Contains, schema.Items2020, schema.UnevaluatedItems, schema.UnevaluatedProperties}
		children = append(children, schema.AllOf...)
		children = append(children, schema.AnyOf...)
		children = append(children, schema.OneOf...)
		children = append(children, schema.PrefixItems...)
		for _, child := range schema.Properties {
			children = append(children, child)
		}
		for name, child := range schema.DependentSchemas {
			node.scanSteps++
			node.scanBytes += len(name) + 1
			children = append(children, child)
		}
		for name := range schema.DependentRequired {
			node.scanSteps++
			node.scanBytes += len(name) + 1
		}
		for expression, child := range schema.PatternProperties {
			compiled, ok := expression.(*regexp.Regexp)
			weight := programs[expression.String()]
			if !ok || weight <= 0 {
				return false
			}
			node.patterns = append(node.patterns, validationCostPattern{compiled, child, weight})
			children = append(children, child)
		}
		sort.Slice(node.patterns, func(i, j int) bool { return node.patterns[i].regexp.String() < node.patterns[j].regexp.String() })
		switch additional := schema.AdditionalProperties.(type) {
		case nil, bool:
		case *jsonschema.Schema:
			children = append(children, additional)
		default:
			return false
		}
		visiting[schema] = true
		for _, child := range children {
			if !visit(child) {
				return false
			}
		}
		delete(visiting, schema)
		plan.nodes[schema] = node
		return true
	}
	if !visit(root) {
		return nil
	}
	return plan
}

// Preserve independent limits for the original operation classes. Indexing
// has its own bound; its work is also covered by the root's subtree charges.
type validationCostMeter struct {
	indexSteps, indexWork           int
	evaluationSteps, evaluationWork int
	patternSteps, patternWork       int
	uniqueSteps, uniqueWork         int
	assertionSteps, assertionWork   int
	regexpWork                      int
}

func costCharge(total *int, amount, limit int, operation string) error {
	if amount < 0 || *total < 0 || *total > limit || amount > limit-*total {
		return fmt.Errorf("%s evaluation exceeds %d", operation, limit)
	}
	*total += amount
	return nil
}

func costProduct(left, right, limit int) (int, bool) {
	if left < 0 || right < 0 || limit < 0 || (right != 0 && left > limit/right) {
		return 0, false
	}
	return left * right, true
}

func costChargeProduct(total *int, left, right, limit int, operation string) error {
	amount, ok := costProduct(left, right, limit)
	if !ok {
		return fmt.Errorf("%s evaluation exceeds %d", operation, limit)
	}
	return costCharge(total, amount, limit, operation)
}

type validationCostInstance struct {
	kind      jsondocument.Kind
	text      string
	nodes     int
	bytes     int
	pathBytes int
	sortWork  int
	children  []*validationCostInstance
	members   []validationCostMember
	lookup    map[string]*validationCostInstance
}

type validationCostMember struct {
	name  string
	value *validationCostInstance
	key   *validationCostInstance
}

func costStringBytes(text string) int {
	size := len(text) + 2
	for index := 0; index < len(text); index++ {
		switch text[index] {
		case '"', '\\', '\b', '\f', '\n', '\r', '\t':
			size++
		default:
			if text[index] < 0x20 {
				size += 5
			}
		}
	}
	return size
}

// This mirrors the strict parser's number-length calculation without creating
// canonical child documents. The original numeric token still reaches Validate.
func costNumberBytes(token string) int {
	negative := strings.HasPrefix(token, "-")
	if negative {
		token = token[1:]
	}
	exponent := 0
	if index := strings.IndexAny(token, "eE"); index >= 0 {
		exponent, _ = strconv.Atoi(token[index+1:])
		token = token[:index]
	}
	integer := len(token)
	if index := strings.IndexByte(token, '.'); index >= 0 {
		integer = index
	}
	leading, digits, trailing := 0, 0, 0
	nonzero := false
	for index := 0; index < len(token); index++ {
		if token[index] == '.' {
			continue
		}
		digits++
		if token[index] == '0' {
			if !nonzero {
				leading++
			}
			trailing++
		} else {
			nonzero, trailing = true, 0
		}
	}
	if !nonzero {
		return 1
	}
	significant := digits - leading - trailing
	decimal := integer + exponent - leading
	size := significant
	switch {
	case decimal <= 0:
		size += 2 - decimal
	case decimal >= significant:
		size = decimal
	default:
		size++
	}
	if negative {
		size++
	}
	return size
}

func costCappedSum(left, right int) int {
	if left > MaxSchemaValidationWork || right > MaxSchemaValidationWork-left {
		return MaxSchemaValidationWork + 1
	}
	return left + right
}

func (meter *validationCostMeter) index(value jsondocument.Value, pathBytes int) (*validationCostInstance, error) {
	if err := costCharge(&meter.indexSteps, 1, MaxSchemaValidationSteps, "schema/payload"); err != nil {
		return nil, err
	}
	node := &validationCostInstance{kind: value.Kind(), nodes: 1, pathBytes: pathBytes}
	ownBytes := 0
	switch value.Kind() {
	case jsondocument.Null:
		ownBytes = 4
	case jsondocument.Boolean:
		ownBytes = 5 // false is the longer spelling
	case jsondocument.Number:
		token, _ := value.NumberToken()
		ownBytes = max(len(token), costNumberBytes(token))
	case jsondocument.String:
		node.text, _ = value.Text()
		ownBytes = costStringBytes(node.text)
	case jsondocument.Array:
		width, _ := value.ArrayLength()
		if width > MaxSchemaValidationSteps-meter.indexSteps {
			return nil, fmt.Errorf("schema/payload evaluation exceeds %d", MaxSchemaValidationSteps)
		}
		ownBytes = 2 + max(0, width-1)
		if err := costCharge(&meter.indexWork, ownBytes, MaxSchemaValidationWork, "schema/payload byte"); err != nil {
			return nil, err
		}
		node.children = make([]*validationCostInstance, 0, width)
		for index := 0; index < width; index++ {
			child, _ := value.Element(index)
			indexed, err := meter.index(child, pathBytes+1+len(strconv.Itoa(index)))
			if err != nil {
				return nil, err
			}
			node.children = append(node.children, indexed)
			node.nodes += indexed.nodes
			node.bytes += indexed.bytes
			node.sortWork = costCappedSum(node.sortWork, indexed.sortWork)
		}
		node.bytes += ownBytes
		return node, nil
	case jsondocument.Object:
		width, _ := value.ObjectLength()
		if width > MaxSchemaValidationSteps-meter.indexSteps {
			return nil, fmt.Errorf("schema/payload evaluation exceeds %d", MaxSchemaValidationSteps)
		}
		ownBytes = 2 + max(0, width-1)
		if err := costCharge(&meter.indexWork, ownBytes, MaxSchemaValidationWork, "schema/payload byte"); err != nil {
			return nil, err
		}
		node.members = make([]validationCostMember, 0, width)
		node.lookup = make(map[string]*validationCostInstance, width)
		keyBytes := 0
		for index := 0; index < width; index++ {
			member, _ := value.MemberAt(index)
			keySize := costStringBytes(member.Name)
			if err := costCharge(&meter.indexWork, keySize+1, MaxSchemaValidationWork, "schema/payload byte"); err != nil {
				return nil, err
			}
			escaped := len(member.Name) + strings.Count(member.Name, "~") + strings.Count(member.Name, "/")
			indexed, err := meter.index(member.Value, pathBytes+1+escaped)
			if err != nil {
				return nil, err
			}
			key := &validationCostInstance{kind: jsondocument.String, text: member.Name, nodes: 1, bytes: keySize, pathBytes: pathBytes}
			node.members = append(node.members, validationCostMember{member.Name, indexed, key})
			node.lookup[member.Name] = indexed
			node.nodes += indexed.nodes
			node.bytes += indexed.bytes + keySize + 1
			keyBytes += keySize
			node.sortWork = costCappedSum(node.sortWork, indexed.sortWork)
		}
		// Hashing unique object items sorts their property names. Reserve both
		// bounded partition passes and the heapsort fallback's comparisons,
		// including repeated long common prefixes, beyond collision work.
		levels := 0
		for size := width; size > 1; size = (size + 1) / 2 {
			levels++
		}
		sortWork, ok := costProduct(keyBytes, 8*levels, MaxSchemaValidationWork)
		if !ok {
			sortWork = MaxSchemaValidationWork + 1
		}
		node.sortWork = costCappedSum(node.sortWork, sortWork)
		node.bytes += ownBytes
		return node, nil
	}
	if err := costCharge(&meter.indexWork, ownBytes, MaxSchemaValidationWork, "schema/payload byte"); err != nil {
		return nil, err
	}
	node.bytes = ownBytes
	return node, nil
}

func (plan *validationCostPlan) measure(value jsondocument.Value) (validationCostMeter, error) {
	var meter validationCostMeter
	instance, err := meter.index(value, 0)
	if err == nil {
		err = plan.apply(&meter, plan.root, instance, 0)
	}
	return meter, err
}

func (plan *validationCostPlan) apply(meter *validationCostMeter, schema *jsonschema.Schema, instance *validationCostInstance, selfDepth int) error {
	if schema == nil {
		return nil
	}
	node, covered := plan.nodes[schema]
	if !covered {
		return fmt.Errorf("schema cost plan is incomplete")
	}
	if err := costCharge(&meter.evaluationSteps, instance.nodes, MaxSchemaValidationSteps, "schema/payload"); err != nil {
		return err
	}
	// Subtree bytes bound scans; actual location bytes and same-instance scope
	// depth also reserve error-path allocation and the validator's cycle check.
	for _, amount := range []int{instance.bytes, instance.pathBytes, len(schema.Location), selfDepth} {
		if err := costCharge(&meter.evaluationWork, amount, MaxSchemaValidationWork, "schema/payload byte"); err != nil {
			return err
		}
	}
	if err := costCharge(&meter.assertionSteps, node.assertions.steps, MaxSchemaValidationSteps, "schema assertion"); err != nil {
		return err
	}
	if err := costCharge(&meter.assertionWork, node.assertions.dataBytes, MaxSchemaValidationWork, "schema assertion byte"); err != nil {
		return err
	}
	if err := costChargeProduct(&meter.assertionWork, instance.bytes, node.assertions.comparisons, MaxSchemaValidationWork, "schema assertion byte"); err != nil {
		return err
	}
	if schema.Bool != nil {
		return nil
	}
	if instance.kind == jsondocument.String && node.pattern != 0 {
		if err := costChargeProduct(&meter.regexpWork, node.pattern, 1+len(instance.text), MaxSchemaValidationWork, "regular-expression byte"); err != nil {
			return err
		}
	}
	if instance.kind == jsondocument.Object {
		if err := costCharge(&meter.evaluationSteps, node.scanSteps, MaxSchemaValidationSteps, "schema/payload"); err != nil {
			return err
		}
		if err := costCharge(&meter.evaluationWork, node.scanBytes, MaxSchemaValidationWork, "schema/payload byte"); err != nil {
			return err
		}
		for _, member := range instance.members {
			child, evaluated := schema.Properties[member.name]
			if err := plan.apply(meter, child, member.value, 0); err != nil {
				return err
			}
			for _, pattern := range node.patterns {
				if err := costCharge(&meter.patternSteps, 1, MaxSchemaValidationSteps, "pattern/property"); err != nil {
					return err
				}
				if err := costCharge(&meter.patternWork, 1+len(member.name), MaxSchemaValidationWork, "pattern/property byte"); err != nil {
					return err
				}
				// Pay before this preflight MatchString and reserve the executor's
				// second match even for nonmatching and overlapping patterns.
				if err := costChargeProduct(&meter.regexpWork, pattern.weight, 2*(1+len(member.name)), MaxSchemaValidationWork, "regular-expression byte"); err != nil {
					return err
				}
				if pattern.regexp.MatchString(member.name) {
					evaluated = true
					if err := plan.apply(meter, pattern.schema, member.value, 0); err != nil {
						return err
					}
				}
			}
			if !evaluated {
				if additional, ok := schema.AdditionalProperties.(*jsonschema.Schema); ok {
					if err := plan.apply(meter, additional, member.value, 0); err != nil {
						return err
					}
				}
			}
			if err := plan.apply(meter, schema.PropertyNames, member.key, 0); err != nil {
				return err
			}
			// Conservative annotation bound: every member may be unevaluated.
			if err := plan.apply(meter, schema.UnevaluatedProperties, member.value, 0); err != nil {
				return err
			}
		}
		for name, dependent := range schema.DependentSchemas {
			if _, present := instance.lookup[name]; present {
				if err := plan.apply(meter, dependent, instance, selfDepth+1); err != nil {
					return err
				}
			}
		}
	}
	if instance.kind == jsondocument.Array {
		width := len(instance.children)
		if schema.UniqueItems && width > 1 {
			if err := costChargeProduct(&meter.uniqueWork, instance.bytes, width, MaxSchemaValidationWork, "uniqueItems byte"); err != nil {
				return err
			}
			if width > 20 {
				if err := costCharge(&meter.uniqueWork, instance.bytes, MaxSchemaValidationWork, "uniqueItems byte"); err != nil {
					return err
				}
				if err := costCharge(&meter.uniqueWork, instance.sortWork, MaxSchemaValidationWork, "uniqueItems byte"); err != nil {
					return err
				}
			}
			if err := costChargeProduct(&meter.uniqueSteps, instance.nodes, width, MaxSchemaValidationSteps, "uniqueItems"); err != nil {
				return err
			}
		}
		for index, child := range instance.children {
			items := schema.Items2020
			if index < len(schema.PrefixItems) {
				items = schema.PrefixItems[index]
			}
			for _, applicable := range []*jsonschema.Schema{items, schema.Contains, schema.UnevaluatedItems} {
				if err := plan.apply(meter, applicable, child, 0); err != nil {
					return err
				}
			}
		}
	}
	// Preserve each application, including shared refs, all potentially executed
	// alternatives and both if branches. Never call Validate to select a branch.
	for _, applicable := range []*jsonschema.Schema{schema.Ref, schema.Not} {
		if err := plan.apply(meter, applicable, instance, selfDepth+1); err != nil {
			return err
		}
	}
	for _, alternatives := range [][]*jsonschema.Schema{schema.AllOf, schema.AnyOf, schema.OneOf} {
		for _, applicable := range alternatives {
			if err := plan.apply(meter, applicable, instance, selfDepth+1); err != nil {
				return err
			}
		}
	}
	if schema.If != nil {
		for _, applicable := range []*jsonschema.Schema{schema.If, schema.Then, schema.Else} {
			if err := plan.apply(meter, applicable, instance, selfDepth+1); err != nil {
				return err
			}
		}
	}
	return nil
}
