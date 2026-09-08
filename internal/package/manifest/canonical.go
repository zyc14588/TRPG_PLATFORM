// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest

import (
	"bytes"
	"fmt"
	"strconv"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
)

// CanonicalTOML serializes the normalized document with a fixed field/table
// order, UTF-8, LF endings, and sorted canonical collections.
func CanonicalTOML(document Document) ([]byte, error) {
	if document.Package != nil {
		value, err := NormalizePackage(*document.Package)
		if err != nil {
			return nil, err
		}
		return canonicalPackageTOML(value), nil
	}
	return nil, fmt.Errorf("canonical TOML currently supports package artifacts only")
}

func canonicalPackageTOML(value Package) []byte {
	var out bytes.Buffer
	line := func(key string, raw any) {
		fmt.Fprintf(&out, "%s = %s\n", key, tomlValue(raw))
	}
	line("schema_version", value.SchemaVersion)
	line("artifact_type", "package")
	line("package_id", value.PackageID.String())
	line("package_kind", string(value.PackageKind))
	line("version", value.Version.String())
	line("display_name", value.DisplayName)
	if value.Entrypoint != "" {
		line("entrypoint", value.Entrypoint)
	}
	if value.LuaProfile != "" {
		line("lua_profile", value.LuaProfile)
	}
	if value.HostAPI != nil {
		out.WriteString("\n[host_api]\n")
		line("major", value.HostAPI.Major)
		line("min_minor", value.HostAPI.MinMinor)
		line("max_minor", value.HostAPI.MaxMinor)
	}
	out.WriteString("\n[build]\n")
	line("source", value.Build.Source)
	line("revision", value.Build.Revision)
	line("builder", value.Build.Builder)
	out.WriteString("\n[rights]\n")
	line("authors", value.Rights.Authors)
	line("source", value.Rights.Source)
	if value.Rights.LicenseExpression != "" {
		line("license_expression", value.Rights.LicenseExpression)
	}
	if value.Rights.Statement != "" {
		line("statement", value.Rights.Statement)
	}
	out.WriteString("\n[capabilities]\n")
	required := make([]string, len(value.Capabilities.Required))
	for i, name := range value.Capabilities.Required {
		required[i] = string(name)
	}
	line("required", required)
	for _, optional := range value.Capabilities.Optional {
		out.WriteString("\n[[capabilities.optional]]\n")
		line("name", string(optional.Name))
		line("fallback", optional.Fallback)
	}
	for _, dependency := range value.Dependencies {
		out.WriteString("\n[[dependencies]]\n")
		line("package_id", dependency.PackageID.String())
		line("version", dependency.Version.String())
		line("optional", dependency.Optional)
		line("features", dependency.Features)
	}
	for _, item := range value.Extensions {
		out.WriteString("\n[[extensions]]\n")
		line("namespace", item.Namespace)
		line("required", item.Required)
		line("contract_version", item.ContractVersion)
		line("schema_path", item.SchemaPath)
		line("schema_sha256", item.SchemaSHA256)
		line("payload_path", item.PayloadPath)
		line("host_api_major", item.HostAPIMajor)
		line("host_api_min_minor", item.HostAPIMinMinor)
		line("host_api_max_minor", item.HostAPIMaxMinor)
	}
	return out.Bytes()
}

func tomlValue(value any) string {
	switch value := value.(type) {
	case string:
		return quoteTOML(value)
	case bool:
		return strconv.FormatBool(value)
	case int:
		return strconv.Itoa(value)
	case uint32:
		return strconv.FormatUint(uint64(value), 10)
	case []string:
		result := "["
		for index, item := range value {
			if index > 0 {
				result += ", "
			}
			result += quoteTOML(item)
		}
		return result + "]"
	case []capability.Name:
		values := make([]string, len(value))
		for i := range value {
			values[i] = string(value[i])
		}
		return tomlValue(values)
	default:
		return fmt.Sprint(value)
	}
}

func quoteTOML(value string) string {
	var result strings.Builder
	result.WriteByte('"')
	for _, character := range value {
		switch character {
		case '"', '\\':
			result.WriteByte('\\')
			result.WriteRune(character)
		case '\b':
			result.WriteString(`\b`)
		case '\t':
			result.WriteString(`\t`)
		case '\n':
			result.WriteString(`\n`)
		case '\f':
			result.WriteString(`\f`)
		case '\r':
			result.WriteString(`\r`)
		default:
			if character < 0x20 || character == 0x7f {
				fmt.Fprintf(&result, `\u%04X`, character)
			} else {
				result.WriteRune(character)
			}
		}
	}
	result.WriteByte('"')
	return result.String()
}
