// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package creator implements the package-generic Creator Studio session. It
// delegates archive preservation and JSON schema validation to the shared Host
// model and contains no game, publisher, or rules-specific behavior.
package creator

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

type ErrorCode string

const (
	ErrConflict   ErrorCode = "ERR_CREATOR_CONFLICT"
	ErrNoSession  ErrorCode = "ERR_CREATOR_NO_SESSION"
	ErrTarget     ErrorCode = "ERR_CREATOR_TARGET"
	ErrDurability ErrorCode = "ERR_CREATOR_DURABILITY"
)

type ContractError struct {
	Code   ErrorCode `json:"code"`
	Detail string    `json:"detail"`
}

func (err *ContractError) Error() string { return fmt.Sprintf("%s: %s", err.Code, err.Detail) }

func IsCode(err error, code ErrorCode) bool {
	var contract *ContractError
	return errors.As(err, &contract) && contract.Code == code
}

func creatorError(code ErrorCode, detail string) error {
	return &ContractError{Code: code, Detail: detail}
}

type ExtensionInspection struct {
	Descriptor       extension.Descriptor `json:"descriptor"`
	Status           extension.Status     `json:"status"`
	Editable         bool                 `json:"editable"`
	ReadOnlyReason   string               `json:"read_only_reason,omitempty"`
	CanonicalJSON    string               `json:"canonical_json,omitempty"`
	RawPayloadBase64 string               `json:"raw_payload_base64,omitempty"`
	RawPayloadSHA256 string               `json:"raw_payload_sha256,omitempty"`
	RawPayloadBytes  int                  `json:"raw_payload_bytes,omitempty"`
}

type Inspection struct {
	SourcePath    string                `json:"source_path"`
	ConflictToken string                `json:"conflict_token"`
	ContentHash   string                `json:"content_hash"`
	Extensions    []ExtensionInspection `json:"extensions"`
}

type EditResult struct {
	Namespace     string `json:"namespace"`
	ConflictToken string `json:"conflict_token"`
	ContentHash   string `json:"content_hash"`
	CanonicalJSON string `json:"canonical_json"`
}

type ExportResult struct {
	Path          string `json:"path"`
	ConflictToken string `json:"conflict_token"`
	ArchiveHash   string `json:"archive_hash"`
	ContentHash   string `json:"content_hash"`
}

type exportOperations struct {
	beforeReplace func(temporary, target string) error
	publish       func(temporary, target string) error
	replace       func(temporary, target string) error
	syncDirectory func(directory string) error
}

func defaultExportOperations() exportOperations {
	return exportOperations{publish: atomicPublish, replace: atomicReplace, syncDirectory: syncParentDirectory}
}

// Service owns one serialized Creator session. The retained package model is
// immutable; edits replace the pointer only after validation succeeds.
type Service struct {
	mu            sync.Mutex
	support       extension.Support
	operations    exportOperations
	pkg           *archive.Package
	sourcePath    string
	conflictToken string
}

func NewService() *Service {
	return NewServiceWithSupport(extension.DefaultSupport)
}

func NewServiceWithSupport(support extension.Support) *Service {
	return newServiceWithOperations(support, defaultExportOperations())
}

func newServiceWithOperations(support extension.Support, operations exportOperations) *Service {
	if operations.publish == nil {
		operations.publish = atomicPublish
	}
	if operations.replace == nil {
		operations.replace = atomicReplace
	}
	if operations.syncDirectory == nil {
		operations.syncDirectory = syncParentDirectory
	}
	return &Service{support: support, operations: operations}
}

func (service *Service) ImportArchive(name string) (Inspection, error) {
	absolute, err := filepath.Abs(name)
	if err != nil {
		return Inspection{}, fmt.Errorf("resolve archive path: %w", err)
	}
	pkg, err := archive.ImportFile(absolute, service.support)
	if err != nil {
		return Inspection{}, err
	}
	token, exists := pkg.SourceArchiveHash()
	if !exists {
		return Inspection{}, fmt.Errorf("imported archive has no exact source token")
	}
	service.mu.Lock()
	defer service.mu.Unlock()
	service.pkg = pkg
	service.sourcePath = absolute
	service.conflictToken = token.String()
	return service.inspectLocked(), nil
}

func (service *Service) Inspect() (Inspection, error) {
	service.mu.Lock()
	defer service.mu.Unlock()
	if service.pkg == nil {
		return Inspection{}, creatorError(ErrNoSession, "no archive is imported")
	}
	return service.inspectLocked(), nil
}

func (service *Service) inspectLocked() Inspection {
	result := Inspection{
		SourcePath: service.sourcePath, ConflictToken: service.conflictToken,
		ContentHash: service.pkg.ContentHash().String(),
		Extensions:  make([]ExtensionInspection, 0, len(service.pkg.Extensions())),
	}
	for _, document := range service.pkg.Extensions() {
		item := ExtensionInspection{
			Descriptor: document.Descriptor, Status: document.Status,
			Editable: document.Status == extension.Supported, ReadOnlyReason: document.ReadOnlyReason,
		}
		if item.Editable {
			item.CanonicalJSON = string(document.CanonicalPayload())
		} else {
			raw := document.PayloadBytes()
			digest := sha256.Sum256(raw)
			item.RawPayloadBase64 = base64.StdEncoding.EncodeToString(raw)
			item.RawPayloadSHA256 = "sha256:" + hex.EncodeToString(digest[:])
			item.RawPayloadBytes = len(raw)
		}
		result.Extensions = append(result.Extensions, item)
	}
	return result
}

// Edit requires the exact source archive token on every call. It re-imports
// that source before applying the edit, while retaining earlier validated
// session edits in the immutable in-memory package model.
func (service *Service) Edit(conflictToken, namespace, jsonText string) (EditResult, error) {
	service.mu.Lock()
	defer service.mu.Unlock()
	if service.pkg == nil {
		return EditResult{}, creatorError(ErrNoSession, "no archive is imported")
	}
	if conflictToken != service.conflictToken {
		return EditResult{}, creatorError(ErrConflict, "archive conflict token does not match the session")
	}
	if len(namespace) == 0 || len(namespace) > extension.MaxNamespaceBytes {
		return EditResult{}, &extension.ContractError{
			Code: extension.ErrInvalid, Detail: "extension namespace is empty or exceeds 128 bytes",
		}
	}
	if len(jsonText) > extension.MaxPayloadBytes {
		return EditResult{}, &extension.ContractError{
			Code: extension.ErrInvalid, Namespace: namespace,
			Detail: fmt.Sprintf("payload exceeds %d bytes", extension.MaxPayloadBytes),
		}
	}
	if err := service.verifySourceLocked(); err != nil {
		return EditResult{}, err
	}
	edited, err := service.pkg.ReplaceExtension(namespace, []byte(jsonText))
	if err != nil {
		return EditResult{}, err
	}
	entry := ""
	for _, document := range edited.Extensions() {
		if document.Descriptor.Namespace == namespace {
			entry = string(document.CanonicalPayload())
			break
		}
	}
	service.pkg = edited
	return EditResult{
		Namespace: namespace, ConflictToken: service.conflictToken,
		ContentHash: edited.ContentHash().String(), CanonicalJSON: entry,
	}, nil
}

func (service *Service) verifySourceLocked() error {
	current, err := archive.ImportFile(service.sourcePath, service.support)
	if err != nil {
		return creatorError(ErrConflict, "source archive is unavailable or changed")
	}
	token, exists := current.SourceArchiveHash()
	if !exists || token.String() != service.conflictToken {
		return creatorError(ErrConflict, "source archive changed since import")
	}
	return nil
}

func (service *Service) Export(conflictToken, targetName string) (ExportResult, error) {
	service.mu.Lock()
	defer service.mu.Unlock()
	if service.pkg == nil {
		return ExportResult{}, creatorError(ErrNoSession, "no archive is imported")
	}
	if conflictToken != service.conflictToken {
		return ExportResult{}, creatorError(ErrConflict, "archive conflict token does not match the session")
	}
	target, err := filepath.Abs(targetName)
	if err != nil {
		return ExportResult{}, fmt.Errorf("resolve export path: %w", err)
	}
	plan, err := planExportTarget(target, service.sourcePath)
	if err != nil {
		return ExportResult{}, err
	}
	snapshot, err := service.pkg.Export()
	if err != nil {
		return ExportResult{}, err
	}
	validated, err := archive.Import(snapshot, service.support)
	if err != nil || validated.ContentHash() != service.pkg.ContentHash() {
		if err == nil {
			err = fmt.Errorf("content hash changed during in-memory re-import")
		}
		return ExportResult{}, fmt.Errorf("validate generated archive in memory: %w", err)
	}
	temporary, err := writeValidatedTemporary(filepath.Dir(target), snapshot, service.support)
	if err != nil {
		return ExportResult{}, err
	}
	keepTemporary := true
	defer func() {
		if keepTemporary {
			_ = os.Remove(temporary)
		}
	}()
	if service.operations.beforeReplace != nil {
		if err := service.operations.beforeReplace(temporary, target); err != nil {
			return ExportResult{}, fmt.Errorf("before atomic replace: %w", err)
		}
	}
	if err := plan.revalidate(target, service.sourcePath); err != nil {
		return ExportResult{}, err
	}
	// The source hash check is deliberately the last fallible validation before
	// the atomic filesystem commit.
	if err := service.verifySourceLocked(); err != nil {
		return ExportResult{}, err
	}
	commit := service.operations.publish
	verb := "publish"
	if plan.existing {
		commit = service.operations.replace
		verb = "replace"
	}
	if err := commit(temporary, target); err != nil {
		return ExportResult{}, fmt.Errorf("atomic %s export target: %w", verb, err)
	}
	keepTemporary = false
	if err := service.operations.syncDirectory(filepath.Dir(target)); err != nil {
		return ExportResult{}, creatorError(ErrDurability, "atomic replace succeeded but parent-directory durability sync failed")
	}
	finalPackage, err := archive.ImportFile(target, service.support)
	if err != nil {
		return ExportResult{}, creatorError(ErrDurability, "atomic replace succeeded but final archive verification failed")
	}
	finalToken, exists := finalPackage.SourceArchiveHash()
	if !exists || finalToken != snapshot.Hash() || finalPackage.ContentHash() != service.pkg.ContentHash() {
		return ExportResult{}, creatorError(ErrDurability, "atomic replace succeeded but final archive identity differs")
	}
	service.pkg = finalPackage
	service.sourcePath = target
	service.conflictToken = finalToken.String()
	return ExportResult{
		Path: target, ConflictToken: finalToken.String(), ArchiveHash: finalToken.String(),
		ContentHash: finalPackage.ContentHash().String(),
	}, nil
}

type exportTargetPlan struct {
	existing bool
	identity os.FileInfo
}

func planExportTarget(target, source string) (exportTargetPlan, error) {
	directory := filepath.Dir(target)
	directoryInfo, err := os.Lstat(directory)
	if err != nil {
		return exportTargetPlan{}, creatorError(ErrTarget, "export parent directory is unavailable")
	}
	if directoryInfo.Mode()&os.ModeSymlink != 0 || !directoryInfo.IsDir() {
		return exportTargetPlan{}, creatorError(ErrTarget, "export parent must be a real directory")
	}
	info, err := os.Lstat(target)
	if errors.Is(err, os.ErrNotExist) {
		return exportTargetPlan{}, nil
	}
	if err != nil {
		return exportTargetPlan{}, creatorError(ErrTarget, "cannot inspect export target")
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return exportTargetPlan{}, creatorError(ErrTarget, "export target must be absent or the imported source file")
	}
	sourceInfo, err := os.Lstat(source)
	if err != nil || sourceInfo.Mode()&os.ModeSymlink != 0 || !sourceInfo.Mode().IsRegular() || !os.SameFile(info, sourceInfo) {
		return exportTargetPlan{}, creatorError(ErrTarget, "an existing export target must identify the imported source file")
	}
	return exportTargetPlan{existing: true, identity: info}, nil
}

func (plan exportTargetPlan) revalidate(target, source string) error {
	current, err := planExportTarget(target, source)
	if err != nil {
		return err
	}
	if current.existing != plan.existing {
		return creatorError(ErrTarget, "export target changed before atomic commit")
	}
	if plan.existing && !os.SameFile(plan.identity, current.identity) {
		return creatorError(ErrTarget, "export target identity changed before atomic commit")
	}
	return nil
}

func writeValidatedTemporary(directory string, snapshot archive.Snapshot, support extension.Support) (string, error) {
	file, err := os.CreateTemp(directory, ".creator-studio-*.tmp")
	if err != nil {
		return "", fmt.Errorf("create export temporary: %w", err)
	}
	name := file.Name()
	clean := true
	defer func() {
		_ = file.Close()
		if clean {
			_ = os.Remove(name)
		}
	}()
	if _, err := bytes.NewReader(snapshot.Bytes()).WriteTo(file); err != nil {
		return "", fmt.Errorf("write export temporary: %w", err)
	}
	if err := file.Sync(); err != nil {
		return "", fmt.Errorf("sync export temporary: %w", err)
	}
	if err := file.Close(); err != nil {
		return "", fmt.Errorf("close export temporary: %w", err)
	}
	validated, err := archive.ImportFile(name, support)
	if err != nil {
		return "", fmt.Errorf("re-import export temporary: %w", err)
	}
	token, exists := validated.SourceArchiveHash()
	if !exists || token != snapshot.Hash() {
		return "", fmt.Errorf("export temporary hash differs from generated snapshot")
	}
	clean = false
	return name, nil
}
