// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package capability implements the fail-closed capability intersection used
// by package manifests. It does not grant or execute Host API operations.
package capability

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strings"
)

var namePattern = regexp.MustCompile(`^[a-z][a-z0-9]*(?:[.-][a-z0-9]+)*$`)

// Name is a declared package capability name such as host.state.
type Name string

const (
	HostEvent Name = "host.event"
	HostLog   Name = "host.log"
	HostState Name = "host.state"
)

// registeredNames is the single version-one capability registry. New names
// require an explicit contract change; syntactically valid strings are not
// capabilities merely because every grant layer repeats them.
var registeredNames = []Name{
	HostEvent,
	HostLog,
	HostState,
}

// RegisteredNames returns the canonical registry in stable order.
func RegisteredNames() []Name {
	return append([]Name(nil), registeredNames...)
}

func ParseName(value string) (Name, error) {
	if len(value) > 128 || !namePattern.MatchString(value) {
		return "", fmt.Errorf("capability %q is not a canonical capability name", value)
	}
	name := Name(value)
	for _, registered := range registeredNames {
		if name == registered {
			return name, nil
		}
	}
	return "", fmt.Errorf("capability %q is not registered", value)
}

// Optional declares the deterministic fallback used when an optional
// capability is not granted.
type Optional struct {
	Name     Name   `json:"name"`
	Fallback string `json:"fallback"`
}

// OptionalSpec is the string form accepted from manifests.
type OptionalSpec struct {
	Name     string
	Fallback string
}

// Declaration is a normalized required/optional capability declaration.
type Declaration struct {
	Required []Name     `json:"required"`
	Optional []Optional `json:"optional"`
}

// NewDeclaration validates capability names, rejects duplicates across both
// classes, requires explicit optional fallbacks, and returns stable ordering.
func NewDeclaration(required []string, optional []OptionalSpec) (Declaration, error) {
	declaration := Declaration{
		Required: make([]Name, 0, len(required)),
		Optional: make([]Optional, 0, len(optional)),
	}
	seen := make(map[Name]struct{}, len(required)+len(optional))
	for _, raw := range required {
		name, err := ParseName(raw)
		if err != nil {
			return Declaration{}, err
		}
		if _, exists := seen[name]; exists {
			return Declaration{}, fmt.Errorf("capability %q is declared more than once", name)
		}
		seen[name] = struct{}{}
		declaration.Required = append(declaration.Required, name)
	}
	for _, raw := range optional {
		name, err := ParseName(raw.Name)
		if err != nil {
			return Declaration{}, err
		}
		if _, exists := seen[name]; exists {
			return Declaration{}, fmt.Errorf("capability %q is declared more than once", name)
		}
		fallback := strings.TrimSpace(raw.Fallback)
		if fallback == "" {
			return Declaration{}, fmt.Errorf("optional capability %q requires a fallback", name)
		}
		seen[name] = struct{}{}
		declaration.Optional = append(declaration.Optional, Optional{Name: name, Fallback: fallback})
	}
	sort.Slice(declaration.Required, func(i, j int) bool { return declaration.Required[i] < declaration.Required[j] })
	sort.Slice(declaration.Optional, func(i, j int) bool { return declaration.Optional[i].Name < declaration.Optional[j].Name })
	return declaration, nil
}

// Validate revalidates a Declaration that may have been constructed directly.
func (declaration Declaration) Validate() error {
	required := make([]string, len(declaration.Required))
	for index, name := range declaration.Required {
		required[index] = string(name)
	}
	optional := make([]OptionalSpec, len(declaration.Optional))
	for index, item := range declaration.Optional {
		optional[index] = OptionalSpec{Name: string(item.Name), Fallback: item.Fallback}
	}
	normalized, err := NewDeclaration(required, optional)
	if err != nil {
		return err
	}
	left, _ := json.Marshal(declaration)
	right, _ := json.Marshal(normalized)
	if string(left) != string(right) {
		return fmt.Errorf("capability declaration is not in canonical order")
	}
	return nil
}

// GrantSet is an immutable-by-API set of capability grants.
type GrantSet struct {
	names map[Name]struct{}
}

func NewGrantSet(values []string) (GrantSet, error) {
	result := GrantSet{names: make(map[Name]struct{}, len(values))}
	for _, value := range values {
		name, err := ParseName(value)
		if err != nil {
			return GrantSet{}, err
		}
		if _, exists := result.names[name]; exists {
			return GrantSet{}, fmt.Errorf("duplicate capability grant %q", name)
		}
		result.names[name] = struct{}{}
	}
	return result, nil
}

func (set GrantSet) Contains(name Name) bool {
	_, exists := set.names[name]
	return exists
}

// Names returns a sorted copy of the grants.
func (set GrantSet) Names() []Name {
	result := make([]Name, 0, len(set.names))
	for name := range set.names {
		result = append(result, name)
	}
	sort.Slice(result, func(i, j int) bool { return result[i] < result[j] })
	return result
}

// TrustLevel is the package trust classification used to select an upper
// capability bound. The policy supplies the grants; no level is all-powerful.
type TrustLevel string

const (
	TrustOfficial          TrustLevel = "official"
	TrustSigned            TrustLevel = "trusted-signed"
	TrustPrivateUnverified TrustLevel = "private-unverified"
	TrustDevelopment       TrustLevel = "development"
)

var trustLevels = []TrustLevel{TrustOfficial, TrustSigned, TrustPrivateUnverified, TrustDevelopment}

func validTrustLevel(level TrustLevel) bool {
	for _, candidate := range trustLevels {
		if level == candidate {
			return true
		}
	}
	return false
}

// TrustPolicy maps every trust level to an explicit upper grant bound.
type TrustPolicy struct {
	grants map[TrustLevel]GrantSet
}

func NewTrustPolicy(values map[TrustLevel][]string) (TrustPolicy, error) {
	policy := TrustPolicy{grants: make(map[TrustLevel]GrantSet, len(trustLevels))}
	for level := range values {
		if !validTrustLevel(level) {
			return TrustPolicy{}, fmt.Errorf("unknown trust level %q", level)
		}
	}
	for _, level := range trustLevels {
		raw, exists := values[level]
		if !exists {
			return TrustPolicy{}, fmt.Errorf("trust policy has no explicit grants for %q", level)
		}
		grants, err := NewGrantSet(raw)
		if err != nil {
			return TrustPolicy{}, fmt.Errorf("trust level %q: %w", level, err)
		}
		policy.grants[level] = grants
	}
	return policy, nil
}

func (policy TrustPolicy) grantsFor(level TrustLevel) (GrantSet, error) {
	grants, exists := policy.grants[level]
	if !exists || !validTrustLevel(level) {
		return GrantSet{}, fmt.Errorf("trust level %q has no capability policy", level)
	}
	return grants, nil
}

// MissingRequiredError reports a fail-closed required-capability denial.
type MissingRequiredError struct {
	Names []Name
}

func (err *MissingRequiredError) Error() string {
	values := make([]string, len(err.Names))
	for index, name := range err.Names {
		values[index] = string(name)
	}
	return "required capabilities are not granted: " + strings.Join(values, ", ")
}

// Fallback records the declared behavior for an unavailable optional grant.
type Fallback struct {
	Name     Name
	Behavior string
}

// Resolution is the immutable result of declared ∩ trust ∩ execution grants.
type Resolution struct {
	Effective GrantSet
	Fallbacks []Fallback
}

// Resolve computes the effective set and never includes an undeclared grant.
// Missing required grants reject the entire resolution.
func Resolve(declaration Declaration, level TrustLevel, policy TrustPolicy, execution GrantSet) (Resolution, error) {
	if err := declaration.Validate(); err != nil {
		return Resolution{}, err
	}
	trust, err := policy.grantsFor(level)
	if err != nil {
		return Resolution{}, err
	}
	effective := GrantSet{names: make(map[Name]struct{})}
	missing := make([]Name, 0)
	for _, name := range declaration.Required {
		if trust.Contains(name) && execution.Contains(name) {
			effective.names[name] = struct{}{}
		} else {
			missing = append(missing, name)
		}
	}
	if len(missing) > 0 {
		return Resolution{}, &MissingRequiredError{Names: missing}
	}
	resolution := Resolution{Effective: effective}
	for _, item := range declaration.Optional {
		if trust.Contains(item.Name) && execution.Contains(item.Name) {
			resolution.Effective.names[item.Name] = struct{}{}
		} else {
			resolution.Fallbacks = append(resolution.Fallbacks, Fallback{Name: item.Name, Behavior: item.Fallback})
		}
	}
	return resolution, nil
}
