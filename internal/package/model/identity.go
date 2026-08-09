// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package model

import (
	"encoding/hex"
	"fmt"
	"regexp"
	"sort"
	"strings"
)

const (
	maxPackageIDLength = 255
	sha256Prefix       = "sha256:"
)

var (
	packageIDPattern = regexp.MustCompile(`^[a-z0-9](?:[a-z0-9.-]{0,126}[a-z0-9])?/[a-z0-9](?:[a-z0-9._-]{0,126}[a-z0-9])?$`)
	semverPattern    = regexp.MustCompile(`^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$`)
)

// PackageID is the immutable publisher-namespace/stable-name identity of a
// package. Display metadata and versions are intentionally absent.
type PackageID string

// ParsePackageID validates the canonical lower-case package ID form.
func ParsePackageID(value string) (PackageID, error) {
	if len(value) == 0 || len(value) > maxPackageIDLength || !packageIDPattern.MatchString(value) {
		return "", fmt.Errorf("package_id %q must be publisher-namespace/stable-name in canonical lower-case form", value)
	}
	return PackageID(value), nil
}

func (id PackageID) String() string { return string(id) }

// Version is a canonical Semantic Versioning 2.0.0 value without a leading v.
type Version string

// ParseVersion validates SemVer, including the no-leading-zero rule for
// numeric prerelease identifiers.
func ParseVersion(value string) (Version, error) {
	match := semverPattern.FindStringSubmatch(value)
	if match == nil {
		return "", fmt.Errorf("version %q is not canonical semantic versioning", value)
	}
	if match[4] != "" {
		for _, identifier := range strings.Split(match[4], ".") {
			if isDecimal(identifier) && len(identifier) > 1 && identifier[0] == '0' {
				return "", fmt.Errorf("version %q has a numeric prerelease identifier with a leading zero", value)
			}
		}
	}
	return Version(value), nil
}

func (version Version) String() string { return string(version) }

func isDecimal(value string) bool {
	if value == "" {
		return false
	}
	for _, character := range value {
		if character < '0' || character > '9' {
			return false
		}
	}
	return true
}

// ContentHash is the canonical algorithm-qualified content hash used by
// package artifacts and exact locks.
type ContentHash string

// ParseContentHash accepts only a lower-case SHA-256 digest. Requiring one
// canonical spelling keeps artifact identities byte-for-byte deterministic.
func ParseContentHash(value string) (ContentHash, error) {
	if !strings.HasPrefix(value, sha256Prefix) {
		return "", fmt.Errorf("content hash must use the sha256: prefix")
	}
	digest := strings.TrimPrefix(value, sha256Prefix)
	if len(digest) != 64 || strings.ToLower(digest) != digest {
		return "", fmt.Errorf("content hash must contain 64 lower-case hexadecimal characters")
	}
	if _, err := hex.DecodeString(digest); err != nil {
		return "", fmt.Errorf("content hash is not hexadecimal: %w", err)
	}
	return ContentHash(value), nil
}

func (hash ContentHash) String() string { return string(hash) }

// BuildProvenance identifies the source and tool that produced an artifact.
type BuildProvenance struct {
	Source   string `json:"source"`
	Revision string `json:"revision"`
	Builder  string `json:"builder"`
}

// NormalizeBuildProvenance validates and trims provenance fields.
func NormalizeBuildProvenance(value BuildProvenance) (BuildProvenance, error) {
	value.Source = strings.TrimSpace(value.Source)
	value.Revision = strings.TrimSpace(value.Revision)
	value.Builder = strings.TrimSpace(value.Builder)
	if value.Source == "" || value.Revision == "" || value.Builder == "" {
		return BuildProvenance{}, fmt.Errorf("build provenance requires source, revision, and builder")
	}
	return value, nil
}

// Rights records the minimum provenance and rights declaration required for
// an artifact. Either LicenseExpression or Statement must be present.
type Rights struct {
	Authors           []string `json:"authors"`
	Source            string   `json:"source"`
	LicenseExpression string   `json:"license_expression,omitempty"`
	Statement         string   `json:"statement,omitempty"`
}

// NormalizeRights validates rights metadata and returns stable author order.
func NormalizeRights(value Rights) (Rights, error) {
	if value.LicenseExpression != "" && strings.TrimSpace(value.LicenseExpression) == "" {
		return Rights{}, fmt.Errorf("rights license expression must be nonblank when provided")
	}
	if value.Statement != "" && strings.TrimSpace(value.Statement) == "" {
		return Rights{}, fmt.Errorf("rights statement must be nonblank when provided")
	}
	value.Source = strings.TrimSpace(value.Source)
	value.LicenseExpression = strings.TrimSpace(value.LicenseExpression)
	value.Statement = strings.TrimSpace(value.Statement)
	if value.Source == "" {
		return Rights{}, fmt.Errorf("rights source is required")
	}
	if value.LicenseExpression == "" && value.Statement == "" {
		return Rights{}, fmt.Errorf("rights require a license expression or rights statement")
	}
	authors, err := normalizedStrings(value.Authors, "rights author")
	if err != nil {
		return Rights{}, err
	}
	if len(authors) == 0 {
		return Rights{}, fmt.Errorf("at least one rights author is required")
	}
	value.Authors = authors
	return value, nil
}

func normalizedStrings(values []string, field string) ([]string, error) {
	result := make([]string, 0, len(values))
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value == "" {
			return nil, fmt.Errorf("%s cannot be empty", field)
		}
		if _, exists := seen[value]; exists {
			return nil, fmt.Errorf("duplicate %s %q", field, value)
		}
		seen[value] = struct{}{}
		result = append(result, value)
	}
	sort.Strings(result)
	return result, nil
}
