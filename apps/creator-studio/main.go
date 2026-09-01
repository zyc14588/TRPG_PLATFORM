// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"

	"github.com/zyc14588/TRPG_PLATFORM/apps/creator-studio/creator"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

var (
	binaryVersion  = "UNSET"
	platformCommit = "UNSET"
	platformTree   = "UNSET"
	buildCommand   = "UNSET"
)

type binaryIdentity struct {
	PlatformCommit string `json:"platform_commit"`
	PlatformTree   string `json:"platform_tree"`
	BinaryVersion  string `json:"binary_version"`
	BuildCommand   string `json:"build_command"`
	BinarySHA256   string `json:"binary_sha256"`
}

type commandPhase struct {
	Name   string `json:"name"`
	Status string `json:"status"`
}

type commandFailure struct {
	Code   string `json:"code"`
	Detail string `json:"detail"`
}

type commandResult struct {
	Operation  string                `json:"operation"`
	Identity   binaryIdentity        `json:"identity"`
	Phases     []commandPhase        `json:"phases,omitempty"`
	Inspection *creator.Inspection   `json:"inspection,omitempty"`
	Edit       *creator.EditResult   `json:"edit,omitempty"`
	Export     *creator.ExportResult `json:"export,omitempty"`
	Reimport   *creator.Inspection   `json:"reimport,omitempty"`
	Failure    *commandFailure       `json:"error,omitempty"`
}

func main() {
	os.Exit(run(os.Args[1:], os.Stdout, os.Stderr))
}

func run(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		if err := runWailsShell(); err != nil {
			fmt.Fprintf(stderr, "creator-studio: start Wails shell: %v\n", err)
			return 1
		}
		return 0
	}
	switch args[0] {
	case "identity":
		if len(args) != 1 {
			fmt.Fprintln(stderr, "usage: creator-studio identity")
			return 2
		}
		identity, err := currentIdentity()
		if err != nil {
			fmt.Fprintf(stderr, "creator-studio: identify executable: %v\n", err)
			return 1
		}
		if err := writeJSON(stdout, identity); err != nil {
			fmt.Fprintf(stderr, "creator-studio: write identity: %v\n", err)
			return 1
		}
		return 0
	case "version":
		if len(args) != 1 {
			fmt.Fprintln(stderr, "usage: creator-studio version")
			return 2
		}
		fmt.Fprintln(stdout, binaryVersion)
		return 0
	case "extension":
		return runExtension(args[1:], stdout, stderr)
	default:
		fmt.Fprintln(stderr, "usage: creator-studio [identity|version|extension inspect|extension edit]")
		return 2
	}
}

func runWailsShell() error {
	assets, err := frontendAssets()
	if err != nil {
		return fmt.Errorf("prepare baseline assets: %w", err)
	}
	return wails.Run(&options.App{
		Title:            "Creator Studio",
		Width:            1200,
		Height:           760,
		MinWidth:         640,
		MinHeight:        480,
		BackgroundColour: &options.RGBA{R: 242, G: 245, B: 249, A: 1},
		AssetServer:      &assetserver.Options{Assets: assets},
		Bind:             []interface{}{creator.NewService()},
	})
}

func currentIdentity() (binaryIdentity, error) {
	digest, err := executableSHA256()
	if err != nil {
		return binaryIdentity{}, err
	}
	return binaryIdentity{
		PlatformCommit: platformCommit,
		PlatformTree:   platformTree,
		BinaryVersion:  binaryVersion,
		BuildCommand:   buildCommand,
		BinarySHA256:   digest,
	}, nil
}

func runExtension(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		fmt.Fprintln(stderr, "usage: creator-studio extension [inspect|edit]")
		return 2
	}
	switch args[0] {
	case "inspect":
		return runExtensionInspect(args[1:], stdout, stderr)
	case "edit":
		return runExtensionEdit(args[1:], stdout, stderr)
	default:
		fmt.Fprintln(stderr, "usage: creator-studio extension [inspect|edit]")
		return 2
	}
}

func runExtensionInspect(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("extension inspect", flag.ContinueOnError)
	flags.SetOutput(stderr)
	archiveName := flags.String("archive", "", "Game Package archive to inspect")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if *archiveName == "" || flags.NArg() != 0 {
		fmt.Fprintln(stderr, "usage: creator-studio extension inspect --archive PATH")
		return 2
	}
	identity, err := currentIdentity()
	if err != nil {
		fmt.Fprintf(stderr, "creator-studio: identify executable: %v\n", err)
		return 1
	}
	service := creator.NewService()
	result := commandResult{Operation: "extension.inspect", Identity: identity}
	imported, err := service.ImportArchive(*archiveName)
	if err != nil {
		return writeCommandFailure(stdout, stderr, &result, "import", err)
	}
	result.Phases = append(result.Phases, commandPhase{Name: "import", Status: "ok"})
	inspected, err := service.Inspect()
	if err != nil {
		return writeCommandFailure(stdout, stderr, &result, "inspect", err)
	}
	result.Phases = append(result.Phases, commandPhase{Name: "inspect", Status: "ok"})
	result.Inspection = &inspected
	// Use the explicit Inspect result so the headless path exercises the same
	// service boundary as Wails, not an archive shortcut.
	_ = imported
	if err := writeJSON(stdout, result); err != nil {
		fmt.Fprintf(stderr, "creator-studio: write result: %v\n", err)
		return 1
	}
	return 0
}

func runExtensionEdit(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("extension edit", flag.ContinueOnError)
	flags.SetOutput(stderr)
	archiveName := flags.String("archive", "", "input Game Package archive")
	namespace := flags.String("namespace", "", "extension namespace")
	jsonName := flags.String("json-file", "", "replacement JSON file")
	outputName := flags.String("output", "", "output Game Package archive")
	conflictToken := flags.String("conflict-token", "", "exact input archive SHA-256")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if *archiveName == "" || *namespace == "" || *jsonName == "" || *outputName == "" || *conflictToken == "" || flags.NArg() != 0 {
		fmt.Fprintln(stderr, "usage: creator-studio extension edit --archive PATH --namespace NAME --json-file PATH --output PATH --conflict-token SHA256")
		return 2
	}
	identity, err := currentIdentity()
	if err != nil {
		fmt.Fprintf(stderr, "creator-studio: identify executable: %v\n", err)
		return 1
	}
	service := creator.NewService()
	result := commandResult{Operation: "extension.edit", Identity: identity}
	if _, err := service.ImportArchive(*archiveName); err != nil {
		return writeCommandFailure(stdout, stderr, &result, "import", err)
	}
	result.Phases = append(result.Phases, commandPhase{Name: "import", Status: "ok"})
	inspection, err := service.Inspect()
	if err != nil {
		return writeCommandFailure(stdout, stderr, &result, "inspect", err)
	}
	result.Inspection = &inspection
	result.Phases = append(result.Phases, commandPhase{Name: "inspect", Status: "ok"})
	payload, err := readBoundedRegularFile(*jsonName, extension.MaxPayloadBytes)
	if err != nil {
		return writeCommandFailure(stdout, stderr, &result, "edit", err)
	}
	edited, err := service.Edit(*conflictToken, *namespace, string(payload))
	if err != nil {
		phase := "edit"
		if extension.IsCode(err, extension.ErrSchemaValidation) || extension.IsCode(err, extension.ErrInvalid) {
			phase = "validate"
		}
		return writeCommandFailure(stdout, stderr, &result, phase, err)
	}
	result.Edit = &edited
	result.Phases = append(result.Phases,
		commandPhase{Name: "edit", Status: "ok"},
		commandPhase{Name: "validate", Status: "ok"},
	)
	exported, err := service.Export(*conflictToken, *outputName)
	if err != nil {
		return writeCommandFailure(stdout, stderr, &result, "export", err)
	}
	result.Export = &exported
	result.Phases = append(result.Phases, commandPhase{Name: "export", Status: "ok"})
	reimported, err := service.ImportArchive(exported.Path)
	if err != nil {
		return writeCommandFailure(stdout, stderr, &result, "reimport", err)
	}
	result.Reimport = &reimported
	result.Phases = append(result.Phases, commandPhase{Name: "reimport", Status: "ok"})
	if err := writeJSON(stdout, result); err != nil {
		fmt.Fprintf(stderr, "creator-studio: write result: %v\n", err)
		return 1
	}
	return 0
}

func writeCommandFailure(stdout, stderr io.Writer, result *commandResult, phase string, err error) int {
	result.Phases = append(result.Phases, commandPhase{Name: phase, Status: "error"})
	result.Failure = structuredFailure(err)
	if writeErr := writeJSON(stdout, *result); writeErr != nil {
		fmt.Fprintf(stderr, "creator-studio: write error result: %v\n", writeErr)
	}
	return 1
}

func structuredFailure(err error) *commandFailure {
	var creatorContract *creator.ContractError
	if errors.As(err, &creatorContract) {
		return &commandFailure{Code: string(creatorContract.Code), Detail: creatorContract.Detail}
	}
	var extensionContract *extension.ContractError
	if errors.As(err, &extensionContract) {
		return &commandFailure{Code: string(extensionContract.Code), Detail: extensionContract.Detail}
	}
	return &commandFailure{Code: "ERR_CREATOR_OPERATION", Detail: err.Error()}
}

func writeJSON(output io.Writer, value any) error {
	encoder := json.NewEncoder(output)
	encoder.SetEscapeHTML(false)
	return encoder.Encode(value)
}

func readBoundedRegularFile(name string, maximum int) ([]byte, error) {
	pathInfo, err := os.Lstat(name)
	if err != nil {
		return nil, fmt.Errorf("inspect JSON input: %w", err)
	}
	if err := validateInputFileInfo(pathInfo, maximum); err != nil {
		return nil, err
	}
	file, err := os.Open(name)
	if err != nil {
		return nil, fmt.Errorf("open JSON input: %w", err)
	}
	defer file.Close()
	openedInfo, err := file.Stat()
	if err != nil {
		return nil, fmt.Errorf("stat JSON input: %w", err)
	}
	if err := validateInputFileInfo(openedInfo, maximum); err != nil {
		return nil, err
	}
	if !os.SameFile(pathInfo, openedInfo) {
		return nil, errors.New("JSON input changed while opening")
	}
	data := make([]byte, int(openedInfo.Size()))
	if _, err := io.ReadFull(file, data); err != nil {
		return nil, fmt.Errorf("read exact JSON input: %w", err)
	}
	var trailer [1]byte
	count, trailerErr := file.Read(trailer[:])
	if count != 0 || trailerErr != io.EOF {
		return nil, errors.New("JSON input grew while reading")
	}
	afterInfo, err := file.Stat()
	if err != nil {
		return nil, fmt.Errorf("restat JSON input: %w", err)
	}
	pathAfter, err := os.Lstat(name)
	if err != nil {
		return nil, fmt.Errorf("reinspect JSON input: %w", err)
	}
	if err := validateInputFileInfo(afterInfo, maximum); err != nil {
		return nil, err
	}
	if err := validateInputFileInfo(pathAfter, maximum); err != nil {
		return nil, err
	}
	if !os.SameFile(openedInfo, afterInfo) || !os.SameFile(afterInfo, pathAfter) ||
		openedInfo.Size() != afterInfo.Size() || afterInfo.Size() != pathAfter.Size() ||
		!openedInfo.ModTime().Equal(afterInfo.ModTime()) || !afterInfo.ModTime().Equal(pathAfter.ModTime()) {
		return nil, errors.New("JSON input changed while reading")
	}
	return data, nil
}

func validateInputFileInfo(info os.FileInfo, maximum int) error {
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return errors.New("JSON input must be a regular file, not a symlink or special file")
	}
	if info.Size() < 0 || info.Size() > int64(maximum) {
		return fmt.Errorf("JSON input exceeds %d bytes", maximum)
	}
	return nil
}

func executableSHA256() (string, error) {
	name, err := os.Executable()
	if err != nil {
		return "", err
	}
	name, err = filepath.EvalSymlinks(name)
	if err != nil {
		return "", fmt.Errorf("resolve executable path: %w", err)
	}
	pathBefore, err := os.Lstat(name)
	if err != nil {
		return "", fmt.Errorf("inspect executable path: %w", err)
	}
	if !pathBefore.Mode().IsRegular() {
		return "", errors.New("executable path is not a regular file")
	}
	file, err := os.Open(name)
	if err != nil {
		return "", fmt.Errorf("open executable: %w", err)
	}
	defer file.Close()
	opened, err := file.Stat()
	if err != nil {
		return "", fmt.Errorf("stat opened executable: %w", err)
	}
	if !opened.Mode().IsRegular() || !os.SameFile(pathBefore, opened) {
		return "", errors.New("executable changed while opening")
	}
	digest := sha256.New()
	buffer := make([]byte, 64<<10)
	if _, err := io.CopyBuffer(digest, file, buffer); err != nil {
		return "", fmt.Errorf("hash executable: %w", err)
	}
	openedAfter, err := file.Stat()
	if err != nil {
		return "", fmt.Errorf("restat opened executable: %w", err)
	}
	pathAfter, err := os.Lstat(name)
	if err != nil {
		return "", fmt.Errorf("reinspect executable path: %w", err)
	}
	if !openedAfter.Mode().IsRegular() || !pathAfter.Mode().IsRegular() ||
		!os.SameFile(opened, openedAfter) || !os.SameFile(openedAfter, pathAfter) ||
		opened.Size() != openedAfter.Size() || openedAfter.Size() != pathAfter.Size() ||
		!opened.ModTime().Equal(openedAfter.ModTime()) || !openedAfter.ModTime().Equal(pathAfter.ModTime()) {
		return "", errors.New("executable changed while hashing")
	}
	return "sha256:" + hex.EncodeToString(digest.Sum(nil)), nil
}
